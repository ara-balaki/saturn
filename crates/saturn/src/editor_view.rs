//! The editor panel: open files as tabs in its strip, the active one in its well, with syntax
//! highlighting from gpui-component's tree-sitter editor.

use std::path::{Path, PathBuf};

use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
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

#[derive(Default)]
pub struct EditorView {
    files: Vec<OpenFile>,
    active: usize,
    /// Why the last file could not be opened or saved, shown in the strip until the next action.
    problem: Option<String>,
}

impl EditorView {
    /// Opens `path` in a tab, or brings its tab forward if it is already open.
    pub fn open(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        self.problem = None;
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
        self.files.push(OpenFile { path: path.to_owned(), state, saved: text, _changes: changes });
        self.active = self.files.len() - 1;
        cx.notify();
    }

    /// Writes the active file to disk.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.files.get_mut(self.active) else { return };
        let text = file.state.read(cx).value().to_string();
        match std::fs::write(&file.path, &text) {
            Ok(()) => {
                file.saved = text;
                self.problem = None;
            }
            Err(error) => self.problem = Some(format!("Could not save {}: {error}", file.name())),
        }
        cx.notify();
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

    fn render_tab(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let file = &self.files[index];
        let selected = index == self.active;
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
                cx.notify();
            }))
            .into_any_element()
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(active) = self.files.get(self.active) else {
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
        let state = active.state.clone();
        chrome::panel()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(2.))
                    .h(len(chrome::CONTROL_HEIGHT))
                    .children((0..self.files.len()).map(|index| self.render_tab(index, cx)))
                    .child(div().flex_1())
                    .children(self.problem.clone().map(|problem| {
                        div()
                            .px(len(space::SPACE_3))
                            .text_size(px(12.))
                            .text_color(theme::color(color::RED))
                            .child(problem)
                    })),
            )
            .child(chrome::well().child(Editor::new(&state).appearance(false).bordered(false).size_full()))
            .into_any_element()
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// The tree-sitter grammar for a file, by its name. `.env` files read well as shell.
fn language_for(path: &Path) -> &'static str {
    let name = path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name.starts_with(".env") || path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("env")) {
        return "bash";
    }
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or_default() {
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "json" | "jsonc" => "json",
        "rs" => "rust",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "mdx" => "markdown",
        "css" | "scss" => "css",
        "html" | "htm" => "html",
        "sh" | "bash" | "zsh" => "bash",
        "py" => "python",
        "go" => "go",
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
        assert_eq!(language_for(Path::new("/p/LICENSE")), "text");
    }
}
