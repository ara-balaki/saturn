//! The editor panel: open files as tabs in its strip, the active one in its well, with syntax
//! highlighting from gpui-component's tree-sitter editor. A Changes tab, when open, reviews the
//! repository's diff with gpui-component's diff view.

use std::path::{Path, PathBuf};

use gpui_kit::component::diff::{Diff, DiffFile, DiffMode, DiffState};
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
use saturn_core::format;
use saturn_core::git::Scope;
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};

/// Files larger than this open read-only elsewhere; the editor keeps the whole text in memory.
const MAX_SIZE: u64 = 4 * 1024 * 1024;

struct OpenFile {
    path: PathBuf,
    state: Entity<EditorState>,
    /// The text as it is on disk, to tell whether there are unsaved changes.
    saved: String,
    /// The file changed on disk while it had unsaved changes here; the user picks which to keep.
    changed_on_disk: bool,
    _changes: Subscription,
}

impl OpenFile {
    fn name(&self) -> String {
        self.path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }

    fn dirty(&self, cx: &gpui_kit::App) -> bool {
        self.state.read(cx).value().as_ref() != self.saved
    }
}

/// The review of a scope's changes, in the Changes tab.
struct Review {
    state: Entity<DiffState>,
    scope: Scope,
}

#[derive(Default)]
pub struct EditorView {
    files: Vec<OpenFile>,
    active: usize,
    review: Option<Review>,
    /// Whether the Changes tab is in front of the files.
    showing_review: bool,
    /// Why the last file could not be opened or saved, shown in the strip until the next action.
    problem: Option<String>,
}

impl EditorView {
    /// Opens `path` in a tab, or brings its tab forward if it is already open.
    pub fn open(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        self.problem = None;
        self.showing_review = false;
        if let Some(index) = self.files.iter().position(|file| file.path == path) {
            self.active = index;
            cx.notify();
            return;
        }
        let text = match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_SIZE => Err(format!("{} is too large to edit here", display_name(path))),
            Ok(_) => std::fs::read_to_string(path).map_err(|_| format!("{} is not a text file", display_name(path))),
            Err(error) => Err(format!("{}: {error}", display_name(path))),
        };
        let text = match text {
            Ok(text) => text,
            Err(problem) => {
                self.problem = Some(problem);
                cx.notify();
                return;
            }
        };
        let language = language_for(path);
        let state = cx.new(|cx| {
            let mut state = EditorState::new(window, cx).language(language).searchable(true);
            state.set_value(text.clone(), window, cx);
            state
        });
        let changes = cx.subscribe(&state, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        self.files.push(OpenFile {
            path: path.to_owned(),
            state,
            saved: text,
            changed_on_disk: false,
            _changes: changes,
        });
        self.active = self.files.len() - 1;
        cx.notify();
    }

    /// Files changed on disk: an open copy without unsaved changes reloads; one with them asks.
    pub fn files_changed(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        for file in &mut self.files {
            if !paths.contains(&file.path) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file.path) else { continue };
            if text == file.saved {
                continue;
            }
            if file.state.read(cx).value().as_ref() == file.saved {
                file.state.update(cx, |state, cx| state.set_value(text.clone(), window, cx));
                file.saved = text;
            } else {
                file.changed_on_disk = true;
            }
        }
        cx.notify();
    }

    /// Takes the disk's version of the active file, dropping the unsaved changes.
    fn reload_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.files.get_mut(self.active) else { return };
        if let Ok(text) = std::fs::read_to_string(&file.path) {
            file.state.update(cx, |state, cx| state.set_value(text.clone(), window, cx));
            file.saved = text;
        }
        file.changed_on_disk = false;
        cx.notify();
    }

    /// Keeps the unsaved changes; saving will replace what is on disk.
    fn keep_active(&mut self, cx: &mut Context<Self>) {
        if let Some(file) = self.files.get_mut(self.active) {
            file.changed_on_disk = false;
        }
        cx.notify();
    }

    /// The question when the active file changed on disk under unsaved changes.
    fn render_disk_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let file = self.files.get(self.active).filter(|file| file.changed_on_disk && !self.showing_review)?;
        Some(
            div()
                .flex()
                .items_center()
                .gap(len(space::SPACE_1))
                .child(
                    div()
                        .px(len(space::SPACE_2))
                        .text_size(px(12.))
                        .text_color(theme::color(color::AMBER_TEXT))
                        .child(format!("{} changed on disk", file.name())),
                )
                .child(
                    chrome::button("disk-reload", "Reload", None, false)
                        .on_click(cx.listener(|this, _, window, cx| this.reload_active(window, cx))),
                )
                .child(
                    chrome::button("disk-keep", "Keep mine", None, false)
                        .on_click(cx.listener(|this, _, _, cx| this.keep_active(cx))),
                )
                .into_any_element(),
        )
    }

    /// Shows `files` in the Changes tab, scrolled to `path`.
    pub fn review(&mut self, files: Vec<DiffFile>, scope: Scope, path: &str, cx: &mut Context<Self>) {
        let state = if let Some(review) = &mut self.review {
            review.scope = scope;
            review.state.update(cx, |state, cx| state.set_files(files, cx));
            review.state.clone()
        } else {
            let state = cx.new(|cx| DiffState::new(files, cx));
            self.review = Some(Review { state: state.clone(), scope });
            state
        };
        state.update(cx, |state, cx| state.scroll_to_file(path, cx));
        self.showing_review = true;
        cx.notify();
    }

    /// Keeps an open review current while agents keep working.
    pub fn refresh_review(&mut self, files: Vec<DiffFile>, scope: Scope, cx: &mut Context<Self>) {
        if let Some(review) = self.review.as_ref().filter(|review| review.scope == scope) {
            review.state.update(cx, |state, cx| state.set_files(files, cx));
        }
    }

    /// Writes the active file to disk and, if `format` is on, runs the project's formatter on it.
    /// The formatted text comes back through the file watcher, like any change on disk.
    pub fn save(&mut self, root: &Path, format: bool, cx: &mut Context<Self>) {
        if self.showing_review {
            return;
        }
        let Some(file) = self.files.get_mut(self.active) else { return };
        let text = file.state.read(cx).value().to_string();
        match std::fs::write(&file.path, &text) {
            Ok(()) => {
                file.saved = text;
                file.changed_on_disk = false;
                self.problem = None;
                if let Some(formatter) = format.then(|| format::formatter_for(&file.path, root)).flatten() {
                    Self::run_formatter(formatter, cx);
                }
            }
            Err(error) => self.problem = Some(format!("Could not save {}: {error}", file.name())),
        }
        cx.notify();
    }

    /// Runs a formatter off the main thread; a failure shows in the strip with its first line.
    fn run_formatter(formatter: format::Formatter, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let name = formatter.name;
            let outcome = cx
                .background_spawn(async move {
                    let mut command = std::process::Command::new(&formatter.program);
                    command.args(&formatter.args).current_dir(&formatter.dir);
                    // Started from Finder, Saturn's own PATH lacks node and rustfmt; the login
                    // shell's has them.
                    if let Some(path) = crate::LOGIN_PATH.get() {
                        command.env("PATH", path);
                    }
                    command.output()
                })
                .await;
            let problem = match outcome {
                Ok(output) if output.status.success() => None,
                Ok(output) => {
                    let errors = String::from_utf8_lossy(&output.stderr);
                    let first =
                        errors.lines().find(|line| !line.trim().is_empty()).unwrap_or("it exited with an error");
                    Some(format!("{name}: {}", first.trim()))
                }
                Err(error) => Some(format!("{name} could not run: {error}")),
            };
            if problem.is_some() {
                let _ = this.update(cx, |this, cx| {
                    this.problem = problem;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn close(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.files.len() {
            self.files.remove(index);
            if self.active >= self.files.len() {
                self.active = self.files.len().saturating_sub(1);
            }
            cx.notify();
        }
    }

    fn render_review_tab(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let review = self.review.as_ref()?;
        let label = match review.scope {
            Scope::Uncommitted => "Uncommitted changes",
            Scope::Branch => "Branch changes",
        };
        Some(
            chrome::track_item("review-tab", self.showing_review)
                .group("review-tab")
                .pl(len(space::SPACE_3))
                .pr(len(space::SPACE_1))
                .child(chrome::icon("git-compare", 13., theme::color(color::INK_MUTED)))
                .child(label)
                .child(
                    div()
                        .id("review-close")
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .rounded(len(radius::RADIUS_XS))
                        .hover(|s| s.bg(theme::color(color::CONTROL_HOVER)))
                        .child(chrome::icon("x", 11., theme::color(color::INK_MUTED)))
                        .when(!self.showing_review, |slot| {
                            slot.invisible().group_hover("review-tab", gpui_kit::Styled::visible)
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.review = None;
                            this.showing_review = false;
                            cx.notify();
                        })),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.showing_review = true;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    /// Unified or split, and stepping through the changes: the review's own controls.
    fn render_review_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let review = self.review.as_ref().filter(|_| self.showing_review)?;
        let mode = review.state.read(cx).mode();
        let state = review.state.clone();
        let pick = |label: &'static str, target: DiffMode| {
            let state = state.clone();
            chrome::track_item(label, mode == target).px(len(space::SPACE_3)).child(label).on_click(move |_, _, cx| {
                state.update(cx, |state, cx| state.set_mode(target, cx));
            })
        };
        let step = |id: &'static str, icon: &str, next: bool| {
            let state = state.clone();
            chrome::icon_button(id, icon, true).on_click(move |_, _, cx| {
                state.update(cx, |state, cx| if next { state.next_change(cx) } else { state.previous_change(cx) });
            })
        };
        Some(
            div()
                .flex()
                .items_center()
                .gap(len(space::SPACE_1))
                .child(step("previous-change", "chevron-up", false))
                .child(step("next-change", "chevron-down", true))
                .child(
                    chrome::track()
                        .p(px(2.))
                        .child(pick("Unified", DiffMode::Unified))
                        .child(pick("Split", DiffMode::Split)),
                )
                .into_any_element(),
        )
    }

    fn render_tab(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let file = &self.files[index];
        let selected = index == self.active && !self.showing_review;
        let dirty = file.dirty(cx);
        chrome::track_item(("editor-tab", index), selected)
            .group("editor-tab")
            .pl(len(space::SPACE_3))
            .pr(len(space::SPACE_1))
            .child(chrome::icon("file", 13., theme::color(color::INK_MUTED)))
            .child(file.name())
            .child(
                div()
                    .id(("editor-close", index))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(20.))
                    .rounded(len(radius::RADIUS_XS))
                    .hover(|s| s.bg(theme::color(color::CONTROL_HOVER)))
                    .map(|slot| {
                        // An unsaved file shows a dot where its close button is, until hovered.
                        if dirty {
                            slot.child(
                                div()
                                    .size(px(7.))
                                    .rounded_full()
                                    .bg(theme::color(color::INK))
                                    .group_hover("editor-tab", gpui_kit::Styled::invisible),
                            )
                        } else {
                            slot.child(chrome::icon("x", 11., theme::color(color::INK_MUTED))).when(!selected, |slot| {
                                slot.invisible().group_hover("editor-tab", gpui_kit::Styled::visible)
                            })
                        }
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| this.close(index, cx))),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active = index;
                this.showing_review = false;
                cx.notify();
            }))
            .into_any_element()
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body: Option<AnyElement> = if self.showing_review {
            self.review.as_ref().map(|review| Diff::new(&review.state).size_full().into_any_element())
        } else {
            self.files
                .get(self.active)
                .map(|file| Editor::new(&file.state).appearance(false).bordered(false).size_full().into_any_element())
        };
        let Some(body) = body else {
            return chrome::panel()
                .child(
                    chrome::well().flex().items_center().justify_center().child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(len(space::SPACE_2))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::color(color::INK))
                                    .child("No file open"),
                            )
                            .child(
                                div().text_size(px(14.)).text_color(theme::color(color::INK_MUTED)).child(
                                    self.problem.clone().unwrap_or_else(|| {
                                        "Pick a file in the sidebar. ⌘J hides this panel.".to_owned()
                                    }),
                                ),
                            ),
                    ),
                )
                .into_any_element();
        };
        chrome::panel()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(2.))
                    .h(len(chrome::CONTROL_HEIGHT))
                    .children(self.render_review_tab(cx))
                    .children((0..self.files.len()).map(|index| self.render_tab(index, cx)))
                    .child(div().flex_1())
                    .children(self.render_disk_notice(cx))
                    .children(self.render_review_controls(cx))
                    .children(self.problem.clone().map(|problem| {
                        div()
                            .px(len(space::SPACE_3))
                            .text_size(px(12.))
                            .text_color(theme::color(color::RED))
                            .child(problem)
                    })),
            )
            .child(chrome::well().child(body))
            .into_any_element()
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// The tree-sitter grammar for a file: by its whole name first (`Dockerfile`, `.gitignore`), then
/// by its extension. Files without a grammar of their own get the closest one that colours their
/// comments and strings: shell for `.env` and ignore files, TOML for INI-style config.
fn language_for(path: &Path) -> &'static str {
    let name = path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name.starts_with(".env") {
        return "bash";
    }
    match name.as_str() {
        "dockerfile" | "containerfile" | ".gitignore" | ".gitattributes" | ".dockerignore" | ".prettierignore"
        | ".npmignore" | ".zshrc" | ".bashrc" | ".zprofile" | ".bash_profile" | ".profile" => return "bash",
        "makefile" | "gnumakefile" => return "make",
        "gemfile" | "rakefile" | "podfile" | "brewfile" => return "ruby",
        ".editorconfig" | ".npmrc" | ".gitconfig" | "cargo.lock" => return "toml",
        _ if name.starts_with("dockerfile.") => return "bash",
        _ => {}
    }
    let extension = path.extension().map(|ext| ext.to_string_lossy().to_lowercase()).unwrap_or_default();
    match extension.as_str() {
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "json" | "jsonc" | "json5" | "jsonl" | "webmanifest" => "json",
        "rs" => "rust",
        "toml" | "ini" | "cfg" | "conf" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "mdx" | "markdown" => "markdown",
        "css" | "scss" | "less" => "css",
        "html" | "htm" | "xml" | "svg" | "plist" | "vue" => "html",
        "svelte" => "svelte",
        "sh" | "bash" | "zsh" | "env" | "dockerfile" => "bash",
        "py" | "pyi" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "m" | "mm" => "cpp",
        "cs" => "csharp",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "rb" | "rake" | "gemspec" => "ruby",
        "php" => "php",
        "lua" => "lua",
        "zig" => "zig",
        "proto" => "proto",
        "graphql" | "gql" => "graphql",
        "diff" | "patch" => "diff",
        "mk" => "make",
        _ => "text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_follow_the_file_name() {
        assert_eq!(language_for(Path::new("/p/.env")), "bash");
        assert_eq!(language_for(Path::new("/p/.env.local")), "bash");
        assert_eq!(language_for(Path::new("/p/apps/api/orders.ts")), "typescript");
        assert_eq!(language_for(Path::new("/p/App.tsx")), "tsx");
        assert_eq!(language_for(Path::new("/p/Cargo.toml")), "toml");
        assert_eq!(language_for(Path::new("/p/README.MD")), "markdown");
        assert_eq!(language_for(Path::new("/p/.github/workflows/ci.yml")), "yaml");
        assert_eq!(language_for(Path::new("/p/Dockerfile")), "bash");
        assert_eq!(language_for(Path::new("/p/Dockerfile.dev")), "bash");
        assert_eq!(language_for(Path::new("/p/.gitignore")), "bash");
        assert_eq!(language_for(Path::new("/p/Makefile")), "make");
        assert_eq!(language_for(Path::new("/p/Cargo.lock")), "toml");
        assert_eq!(language_for(Path::new("/p/LICENSE")), "text");
    }
}
