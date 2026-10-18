//! The file tree in the sidebar's well: a virtualized list of the open folders' entries, with git
//! badges, kept current by a file watcher.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::{
    AnyElement, AppContext as _, Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px, uniform_list,
};
use notify::Watcher as _;
use saturn_core::files::{FileTree, Ignored, Row};
use saturn_core::git::{self, FileState};
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};

const ROW_HEIGHT: f32 = 26.0;
/// How far each level is indented.
const INDENT: f32 = 14.0;
/// File changes are gathered this long before the tree reads them, so a `git checkout` touching a
/// thousand files refreshes once.
const SETTLE: Duration = Duration::from_millis(150);

pub enum FilesEvent {
    Open(PathBuf),
    /// Git reported a new state: the Changes list and any review should read it again.
    GitChanged,
    /// Files changed on disk (not ignored ones); an open copy may need reloading.
    Changed(Vec<PathBuf>),
}

pub struct FilesView {
    tree: FileTree,
    rows: Vec<Row>,
    changes: HashMap<PathBuf, FileState>,
    selected: Option<PathBuf>,
    ignored: Ignored,
    /// Kept alive for as long as the view watches its root.
    _watcher: Option<notify::RecommendedWatcher>,
}

impl EventEmitter<FilesEvent> for FilesView {}

impl FilesView {
    pub fn new(root: &Path, cx: &mut Context<Self>) -> Self {
        let mut tree = FileTree::new(root.to_owned());
        let rows = tree.rows();
        let (sender, events) = async_channel::unbounded::<PathBuf>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(event) = event {
                for path in event.paths {
                    let _ = sender.send_blocking(path);
                }
            }
        })
        .and_then(|mut watcher| watcher.watch(root, notify::RecursiveMode::Recursive).map(|()| watcher))
        .ok();
        cx.spawn(async move |this, cx| {
            while let Ok(first) = events.recv().await {
                cx.background_executor().timer(SETTLE).await;
                let mut batch = vec![first];
                while let Ok(path) = events.try_recv() {
                    batch.push(path);
                }
                if this.update(cx, |this, cx| this.files_changed(&batch, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let ignored = Ignored::new(root);
        let mut view = Self { tree, rows, changes: HashMap::new(), selected: None, ignored, _watcher: watcher };
        view.refresh_git(cx);
        view
    }

    /// Re-reads the folders whose entries changed, and git's view of everything.
    fn files_changed(&mut self, changed: &[PathBuf], cx: &mut Context<Self>) {
        // Builds write thousands of ignored files (target/, node_modules/); they change nothing
        // the tree shows or git reports.
        let changed: Vec<&PathBuf> = changed.iter().filter(|path| !self.ignored.is_ignored(path)).collect();
        if changed.is_empty() {
            return;
        }
        cx.emit(FilesEvent::Changed(changed.iter().map(|path| (*path).clone()).collect()));
        let folders: HashSet<PathBuf> = changed
            .iter()
            .filter(|path| !path.components().any(|part| part.as_os_str() == ".git"))
            .filter_map(|path| path.parent().map(Path::to_owned))
            .collect();
        if !folders.is_empty() {
            self.tree.invalidate(Some(&folders.into_iter().collect::<Vec<_>>()));
            self.rows = self.tree.rows();
        }
        self.refresh_git(cx);
        cx.notify();
    }

    fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let root = self.tree.root().to_owned();
        cx.spawn(async move |this, cx| {
            let changes = cx.background_spawn(async move { git::status(&root) }).await;
            let _ = this.update(cx, |this, cx| {
                if this.changes != changes {
                    this.changes = changes;
                    cx.emit(FilesEvent::GitChanged);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn click(&mut self, row: &Row, cx: &mut Context<Self>) {
        self.selected = Some(row.path.clone());
        if row.is_dir {
            self.tree.toggle(&row.path);
            self.rows = self.tree.rows();
        } else {
            cx.emit(FilesEvent::Open(row.path.clone()));
        }
        cx.notify();
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = self.rows[index].clone();
        let selected = self.selected.as_deref() == Some(row.path.as_path());
        let state = self.changes.get(&row.path).copied();
        // A closed folder with changes beneath it gets a quiet dot, so nothing hides in it.
        let changed_inside = row.is_dir && !row.expanded && git::has_changes_under(&self.changes, &row.path);
        let name_color = match state {
            Some(FileState::Deleted) => color::INK_FAINT,
            _ if selected => color::INK,
            _ => color::INK_MUTED,
        };
        let icon = match (row.is_dir, row.expanded) {
            (true, true) => "folder-open",
            (true, false) => "folder",
            (false, _) => "file",
        };
        div()
            .id(("file", index))
            .w_full()
            .flex()
            .items_center()
            .gap(len(space::SPACE_2))
            .h(px(ROW_HEIGHT))
            .pl(px(f32::from(u16::try_from(row.depth).unwrap_or(u16::MAX)) * INDENT + 6.))
            .pr(len(space::SPACE_3))
            .rounded(len(radius::RADIUS_SM))
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(theme::color(name_color))
            .when(selected, |row| row.bg(theme::fill_pressed()))
            .when(!selected, |row| row.hover(|s| s.bg(theme::fill_hover())))
            .child(div().w(px(12.)).flex_none().when(row.is_dir, |slot| {
                slot.child(chrome::icon(
                    if row.expanded { "chevron-down" } else { "chevron-right" },
                    12.,
                    theme::color(color::INK_FAINT),
                ))
            }))
            .child(chrome::icon(icon, 14., theme::color(color::INK_MUTED)))
            .child(div().flex_1().min_w_0().truncate().child(row.name.clone()))
            .children(state.map(|state| {
                div()
                    .flex_none()
                    .font_family(saturn_design::font::MONO)
                    .text_size(px(11.))
                    .text_color(theme::color(badge_color(state)))
                    .child(state.badge())
            }))
            .when(changed_inside, |row| {
                row.child(div().flex_none().size(px(5.)).rounded_full().bg(theme::color(color::AMBER_TEXT)))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.click(&row, cx)))
            .into_any_element()
    }
}

fn badge_color(state: FileState) -> saturn_design::Token {
    match state {
        FileState::Modified | FileState::Renamed => color::AMBER_TEXT,
        FileState::Added | FileState::Untracked => color::TERM_GREEN,
        FileState::Deleted | FileState::Conflicted => color::RED,
    }
}

impl Render for FilesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        uniform_list(
            "file-tree",
            self.rows.len(),
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                range.map(|index| this.render_row(index, cx)).collect::<Vec<_>>()
            }),
        )
        .size_full()
        .p(len(space::SPACE_1))
    }
}
