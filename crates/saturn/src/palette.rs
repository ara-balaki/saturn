//! The palettes: Go to file (⌘P) and the command palette (⌘⇧P). One search field over a list,
//! fuzzy-matched as you type; ↑↓ move, Enter picks, Esc closes.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::{
    Action, AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};
use saturn_core::fuzzy;
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};
use crate::workspace::{self, Workspace};

/// How many matches a palette lists: enough to scroll, few enough to draw at once.
const SHOWN: usize = 60;

/// A line in a palette.
struct Entry {
    /// What the search matches: a file's path, a command's name.
    text: String,
    /// What the line says, and in faint type after it.
    label: String,
    detail: Option<String>,
    icon: &'static str,
    /// The action whose shortcut the line shows.
    action: Option<Box<dyn Action>>,
}

type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

pub struct Palette {
    search: Entity<InputState>,
    entries: Vec<Entry>,
    /// The entries that match the search, best first.
    shown: Vec<usize>,
    selected: usize,
    /// What's said while there's nothing to list.
    empty: &'static str,
    pick: Pick,
    _search_events: Subscription,
}

impl Palette {
    fn new(
        placeholder: &'static str,
        empty: &'static str,
        entries: Vec<Entry>,
        pick: Pick,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Screenshots type their query up front.
        let typed = std::env::var("SATURN_PALETTE_QUERY").unwrap_or_default();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(typed));
        search.update(cx, |search, cx| search.focus(window, cx));
        let search_events = cx.subscribe_in(&search, window, |this, _, event: &InputEvent, window, cx| match event {
            InputEvent::Change => this.filter(cx),
            InputEvent::PressEnter { .. } => this.choose(window, cx),
            _ => {}
        });
        let mut palette =
            Self { search, entries, shown: Vec::new(), selected: 0, empty, pick, _search_events: search_events };
        palette.filter(cx);
        palette
    }

    fn set_entries(&mut self, entries: Vec<Entry>, cx: &mut Context<Self>) {
        self.entries = entries;
        self.filter(cx);
    }

    fn filter(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().to_string();
        self.shown = fuzzy::rank(&query, self.entries.iter().map(|entry| entry.text.as_str()), SHOWN);
        self.selected = 0;
        cx.notify();
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&index) = self.shown.get(self.selected) else { return };
        let pick = self.pick.clone();
        window.close_dialog(cx);
        // After the dialog has handed the focus back, so a command reaches the pane it's for.
        window.defer(cx, move |window, cx| pick(index, window, cx));
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        if self.shown.is_empty() {
            return;
        }
        let last = self.shown.len() - 1;
        self.selected = self.selected.saturating_add_signed(by).min(last);
        cx.notify();
    }

    fn render_entry(&self, row: usize, index: usize, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let entry = &self.entries[index];
        let selected = row == self.selected;
        div()
            .id(("entry", row))
            .flex()
            .items_center()
            .gap(len(space::SPACE_3))
            .h(px(32.))
            .px(len(space::SPACE_3))
            .rounded(len(radius::RADIUS_SM))
            .cursor_pointer()
            .text_size(px(14.))
            .when(selected, |line| line.bg(theme::fill_pressed()))
            .when(!selected, |line| line.hover(|s| s.bg(theme::fill_hover())))
            .child(chrome::icon(entry.icon, 14., theme::color(color::INK_MUTED)))
            .child(div().flex_none().text_color(theme::color(color::INK)).child(entry.label.clone()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .text_color(theme::color(color::INK_FAINT))
                    .children(entry.detail.clone()),
            )
            .children(entry.action.as_deref().and_then(|action| Kbd::binding_for_action(action, None, window)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.selected = row;
                this.choose(window, cx);
            }))
            .into_any_element()
    }
}

impl Render for Palette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<AnyElement> = self
            .shown
            .clone()
            .into_iter()
            .enumerate()
            .map(|(row, index)| self.render_entry(row, index, window, cx))
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(len(space::SPACE_2))
            // Before the search field sees them: it would move its caret instead.
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| match event.keystroke.key.as_str() {
                "up" => {
                    this.step(-1, cx);
                    cx.stop_propagation();
                }
                "down" => {
                    this.step(1, cx);
                    cx.stop_propagation();
                }
                _ => {}
            }))
            .child(Input::new(&self.search))
            .child(if rows.is_empty() {
                div()
                    .py(len(space::SPACE_4))
                    .px(len(space::SPACE_3))
                    .text_size(px(14.))
                    .text_color(theme::color(color::INK_MUTED))
                    .child(self.empty)
                    .into_any_element()
            } else {
                div()
                    .id("palette-list")
                    .flex()
                    .flex_col()
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .children(rows)
                    .into_any_element()
            })
    }
}

fn open(title: &'static str, palette: Entity<Palette>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title(title).bg(chrome::dialog_surface()).w(px(640.)).child(palette.clone())
    });
}

/// Go to file: every file in the folder the sidebar shows (what git ignores left out).
pub fn go_to_file(workspace: WeakEntity<Workspace>, root: PathBuf, window: &mut Window, cx: &mut App) {
    let files: Rc<std::cell::RefCell<Vec<PathBuf>>> = Rc::default();
    let pick: Pick = {
        let (files, root) = (files.clone(), root.clone());
        Rc::new(move |index, window, cx| {
            let Some(file) = files.borrow().get(index).cloned() else { return };
            let _ = workspace.update(cx, |workspace, cx| workspace.open_file(&root.join(file), window, cx));
        })
    };
    let palette =
        cx.new(|cx| Palette::new("Search files by name or path", "Reading the files…", Vec::new(), pick, window, cx));
    let weak = palette.downgrade();
    cx.spawn(async move |cx| {
        let found = cx.background_spawn(async move { list_files(&root) }).await;
        let entries = found.iter().map(|file| file_entry(file)).collect();
        *files.borrow_mut() = found;
        let _ = weak.update(cx, |palette, cx| {
            palette.empty = "No file matches.";
            palette.set_entries(entries, cx);
        });
    })
    .detach();
    open("Go to file", palette, window, cx);
}

fn file_entry(file: &Path) -> Entry {
    let label = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let detail = file.parent().map(|folder| folder.display().to_string()).filter(|folder| !folder.is_empty());
    Entry { text: file.display().to_string(), label, detail, icon: "file-code", action: None }
}

/// The files under `root`, relative to it, without what git ignores; at most 100,000.
fn list_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = ignore::WalkBuilder::new(root)
        .require_git(false)
        .hidden(false)
        .filter_entry(|entry| entry.file_name() != ".git" && entry.file_name() != "node_modules")
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.path().strip_prefix(root).ok().map(Path::to_path_buf))
        .take(100_000)
        .collect();
    files.sort();
    files
}

/// The command palette: everything Saturn does, by name, with its shortcut.
pub fn commands(window: &mut Window, cx: &mut App) {
    let commands = workspace::commands();
    let entries = commands
        .iter()
        .map(|(name, icon, action)| Entry {
            text: (*name).to_owned(),
            label: (*name).to_owned(),
            detail: None,
            icon,
            action: Some(action.boxed_clone()),
        })
        .collect();
    let pick: Pick = Rc::new(move |index, window, cx| {
        if let Some((_, _, action)) = commands.get(index) {
            window.dispatch_action(action.boxed_clone(), cx);
        }
    });
    let palette = cx.new(|cx| Palette::new("Type a command", "No command matches.", entries, pick, window, cx));
    open("Commands", palette, window, cx);
}
