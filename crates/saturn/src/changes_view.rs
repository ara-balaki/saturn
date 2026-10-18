//! The Changes tab: what changed in the repository, as one `git diff` parsed into files. The same
//! files feed the list here and the review in the editor panel, so the two never disagree.
//!
//! Two scopes, as review tools for agents offer them: what is not committed yet, and everything the
//! branch changed since it left the default branch (what a pull request would show).

use std::path::{Path, PathBuf};

use gpui_kit::component::diff::{DiffFile, DiffFileStatus};
use gpui_kit::{
    AnyElement, AppContext as _, Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px, uniform_list,
};
use saturn_core::git::{self, Scope};
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};

const ROW_HEIGHT: f32 = 26.0;

pub enum ChangesEvent {
    /// Review the changes, starting at `path`.
    Open { files: Vec<DiffFile>, scope: Scope, path: String },
    /// The changes were read again; an open review should show the new ones.
    Updated { files: Vec<DiffFile>, scope: Scope },
}

pub struct ChangesView {
    repo: PathBuf,
    scope: Scope,
    files: Vec<DiffFile>,
    selected: Option<String>,
    loaded: bool,
}

impl EventEmitter<ChangesEvent> for ChangesView {}

impl ChangesView {
    pub fn new(repo: &Path, cx: &mut Context<Self>) -> Self {
        let mut view =
            Self { repo: repo.to_owned(), scope: Scope::default(), files: Vec::new(), selected: None, loaded: false };
        view.reload(cx);
        view
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.scope != scope {
            self.scope = scope;
            self.reload(cx);
        }
    }

    /// The number of changed files and lines, for the strip above the list.
    pub fn summary(&self) -> Option<(usize, usize, usize)> {
        self.loaded.then(|| {
            let added = self.files.iter().map(DiffFile::additions).sum();
            let removed = self.files.iter().map(DiffFile::deletions).sum();
            (self.files.len(), added, removed)
        })
    }

    /// Reads the changes again, off the main thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let (repo, scope) = (self.repo.clone(), self.scope);
        cx.spawn(async move |this, cx| {
            let files = cx
                .background_spawn(async move { DiffFile::parse(&git::patch(&repo, scope)).unwrap_or_default() })
                .await;
            let _ = this.update(cx, |this, cx| {
                // A scope switched while reading has its own read under way.
                if this.scope == scope {
                    this.files = files;
                    this.loaded = true;
                    cx.emit(ChangesEvent::Updated { files: this.files.clone(), scope });
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn open(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(file) = self.files.get(index) else { return };
        let path = file.path().to_string();
        self.selected = Some(path.clone());
        cx.emit(ChangesEvent::Open { files: self.files.clone(), scope: self.scope, path });
        cx.notify();
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let file = &self.files[index];
        let path = file.path().to_string();
        let (folder, name) = match path.rsplit_once('/') {
            Some((folder, name)) => (folder.to_owned(), name.to_owned()),
            None => (String::new(), path.clone()),
        };
        let selected = self.selected.as_deref() == Some(path.as_str());
        let (badge, badge_color) = match file.status() {
            DiffFileStatus::Added => ("A", color::TERM_GREEN),
            DiffFileStatus::Deleted => ("D", color::RED),
            DiffFileStatus::Renamed | DiffFileStatus::Copied => ("R", color::AMBER_TEXT),
            DiffFileStatus::Conflicted => ("!", color::RED),
            _ => ("M", color::AMBER_TEXT),
        };
        div()
            .id(("change", index))
            .w_full()
            .flex()
            .items_center()
            .gap(len(space::SPACE_2))
            .h(px(ROW_HEIGHT))
            .px(len(space::SPACE_3))
            .rounded(len(radius::RADIUS_SM))
            .cursor_pointer()
            .text_size(px(13.))
            .when(selected, |row| row.bg(theme::color(color::CONTROL_PRESSED)))
            .when(!selected, |row| row.hover(|s| s.bg(theme::color(color::CONTROL))))
            .child(
                div()
                    .w(px(10.))
                    .flex_none()
                    .font_family(saturn_design::font::MONO)
                    .text_size(px(11.))
                    .text_color(theme::color(badge_color))
                    .child(badge),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(len(space::SPACE_2))
                    .child(
                        div()
                            .flex_none()
                            .text_color(theme::color(if selected { color::INK } else { color::INK_BODY }))
                            .child(name),
                    )
                    .child(div().min_w_0().truncate().text_color(theme::color(color::INK_FAINT)).child(folder)),
            )
            .child(stats(file.additions(), file.deletions()))
            .on_click(cx.listener(move |this, _, _, cx| this.open(index, cx)))
            .into_any_element()
    }
}

/// `+12 −3`, in the diff's own colours.
pub fn stats(added: usize, removed: usize) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .gap(len(space::SPACE_1))
        .font_family(saturn_design::font::MONO)
        .text_size(px(11.))
        .when(added > 0, |stats| {
            stats.child(div().text_color(theme::color(color::TERM_GREEN)).child(format!("+{added}")))
        })
        .when(removed > 0, |stats| stats.child(div().text_color(theme::color(color::RED)).child(format!("−{removed}"))))
}

impl Render for ChangesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.loaded && self.files.is_empty() {
            let message = match self.scope {
                Scope::Uncommitted => "Nothing uncommitted.",
                Scope::Branch => "This branch has no changes yet.",
            };
            return div()
                .size_full()
                .p(len(space::SPACE_4))
                .text_size(px(14.))
                .text_color(theme::color(color::INK_MUTED))
                .child(message)
                .into_any_element();
        }
        uniform_list(
            "changes",
            self.files.len(),
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                range.map(|index| this.render_row(index, cx)).collect::<Vec<_>>()
            }),
        )
        .size_full()
        .p(len(space::SPACE_1))
        .into_any_element()
    }
}

/// The scope switch and the totals, for the sidebar's strip while Changes is showing.
pub fn scope_strip(view: &gpui_kit::Entity<ChangesView>, cx: &mut gpui_kit::App) -> impl IntoElement {
    let (scope, summary) = {
        let changes = view.read(cx);
        (changes.scope(), changes.summary())
    };
    let pick = |label: &'static str, target: Scope| {
        let view = view.clone();
        chrome::track_item(label, scope == target).flex_1().px(len(space::SPACE_3)).child(label).on_click(
            move |_, _, cx| {
                view.update(cx, |view, cx| view.set_scope(target, cx));
            },
        )
    };
    div()
        .flex()
        .flex_col()
        .gap(len(space::SPACE_1))
        .child(chrome::track().child(pick("Uncommitted", Scope::Uncommitted)).child(pick("Branch", Scope::Branch)))
        .children(summary.map(|(files, added, removed)| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(len(space::SPACE_3))
                .h(px(20.))
                .text_size(px(12.))
                .text_color(theme::color(color::INK_MUTED))
                .child(match files {
                    1 => "1 file".to_owned(),
                    n => format!("{n} files"),
                })
                .child(stats(added, removed))
        }))
}
