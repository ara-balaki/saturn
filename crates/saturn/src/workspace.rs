//! The window's root: the title bar and the grid of terminal panes, each with tabs.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, AppContext as _, ClickEvent, Context, CursorStyle, DragMoveEvent, Entity,
    FocusHandle, Focusable as _, FontWeight, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, PathPromptOptions, PromptLevel, Render, StatefulInteractiveElement as _, Styled as _, Window,
    actions, bounce, div, ease_in_out, prelude::FluentBuilder as _, px, relative,
};
use saturn_bridge::{Bridge, HookCall};
use saturn_core::agent_state::{AgentState, tab_title};
use saturn_core::layout::{Cell, CellId, Direction, Layout, RowId, TabId};
use saturn_core::session_name::rename_for_prompt;
use saturn_design::{color, radius, shadow, space};
use saturn_terminal::view::{TerminalView, TerminalViewEvent};

use crate::chrome::{self, TileKind};
use crate::settings::{self, Settings};
use crate::theme::{self, len};
use crate::{dock, settings_view};

actions!(
    saturn,
    [
        NewTab,
        SplitRight,
        SplitDown,
        CloseTab,
        NextTab,
        PreviousTab,
        FocusLeft,
        FocusRight,
        FocusUp,
        FocusDown,
        ToggleMaximize,
        NextAttention,
        Paste,
        Copy,
        ToggleSidebar,
        ToggleEditor,
        OpenProject,
        OpenSettings,
        FocusPane1,
        FocusPane2,
        FocusPane3,
        FocusPane4,
        FocusPane5,
        FocusPane6,
        FocusPane7,
        FocusPane8,
        FocusPane9,
    ]
);

#[derive(Clone, Copy)]
pub enum Placement {
    Tab,
    Right,
    Down,
}

const TITLEBAR_HEIGHT: f32 = 44.0;
const TRAFFIC_LIGHTS_WIDTH: f32 = 76.0;
const PANE_HEADER_HEIGHT: f32 = 40.0;
const TAB_HEIGHT: f32 = 28.0;
/// No pane is dragged smaller than this share of its row or of the grid.
const MIN_SHARE: f32 = 0.08;
/// Programs a tab can close without asking, because nothing is lost.
const SHELLS: &[&str] = &["zsh", "-zsh", "bash", "-bash", "fish", "sh", "login"];

/// A tab being dragged to another place in the grid.
#[derive(Clone)]
struct DraggedTab {
    tab: TabId,
    name: String,
}

impl Render for DraggedTab {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(len(space::SPACE_3))
            .h(len(TAB_HEIGHT))
            .flex()
            .items_center()
            .rounded(len(radius::RADIUS_LG))
            .bg(theme::color(color::CONTROL))
            .border_1()
            .border_color(theme::color(color::PURPLE))
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::color(color::INK))
            .font_family(saturn_design::font::SANS)
            .child(self.name.clone())
    }
}

/// A divider in the grid being dragged: between two rows, or between two cells of a row.
#[derive(Clone, Copy)]
enum GridDivider {
    Rows(usize),
    Cells(RowId, usize),
}

struct DividerPreview;

impl Render for DividerPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub struct Workspace {
    project: PathBuf,
    layout: Layout,
    terminals: HashMap<TabId, Entity<TerminalView>>,
    /// Names typed by hand; they win over the terminal's own title.
    names: HashMap<TabId, String>,
    /// Tabs that changed into a state asking for the user while they looked elsewhere.
    unseen: HashSet<TabId>,
    renaming: Option<(TabId, Entity<InputState>)>,
    next_tab: u64,
    focus: FocusHandle,
    sidebar_open: bool,
    editor_open: bool,
    settings: Settings,
    /// Where the Saturn plugin's hooks reach this window; `None` if it could not start.
    bridge: Option<Bridge>,
    /// The last name Saturn gave each tab's Claude session.
    session_names: HashMap<TabId, String>,
    /// The number on the Dock icon right now.
    badge: usize,
}

/// The Saturn plugin (`plugin/` in this repository) that Claude sessions in Saturn's panes load.
fn plugin_dir() -> Option<PathBuf> {
    std::env::var_os("SATURN_PLUGIN_DIR")
        .map(PathBuf::from)
        .or_else(|| Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugin")).canonicalize().ok())
        .filter(|dir| dir.join(".claude-plugin/plugin.json").is_file())
}

/// Starts the hook bridge on a socket only this Saturn instance uses.
fn start_bridge() -> Option<(Bridge, async_channel::Receiver<HookCall>)> {
    let path = std::env::temp_dir().join(format!("saturn-{}.sock", std::process::id()));
    match Bridge::start(&path) {
        Ok(started) => Some(started),
        Err(error) => {
            eprintln!("saturn: hooks can't reach this window ({error}); agent states come from the terminal only");
            None
        }
    }
}

fn state_color(state: AgentState) -> Hsla {
    theme::color(match state {
        AgentState::Shell | AgentState::Idle => color::STATUS_IDLE,
        AgentState::Working => color::STATUS_WORKING,
        AgentState::NeedsYou => color::STATUS_NEEDS_YOU,
        AgentState::YourTurn => color::STATUS_YOUR_TURN,
        AgentState::Error => color::STATUS_ERROR,
    })
}

fn status_dot(state: AgentState) -> impl IntoElement {
    div().size(px(8.)).flex_none().rounded_full().bg(state_color(state))
}

/// `glow-needs-you` as it breathes: `t` from 0 (the token itself) to 1 (wider and brighter).
fn breathing_glow(t: f32) -> Vec<saturn_design::Shadow> {
    let mut glow = shadow::GLOW_NEEDS_YOU.to_vec();
    if let Some(halo) = glow.get_mut(1) {
        halo.blur += 10. * t;
        halo.color.alpha += 0.14 * t;
    }
    glow
}

/// Moves the divider after `index` to `at` (0–1 across the container), keeping both neighbours
/// above the minimum share.
fn move_divider(sizes: &[f32], index: usize, at: f32) -> Vec<f32> {
    let total: f32 = sizes.iter().sum();
    let before: f32 = sizes[..index].iter().sum();
    let pair = sizes[index] + sizes[index + 1];
    let minimum = total * MIN_SHARE;
    let first = (at * total - before).clamp(minimum, (pair - minimum).max(minimum));
    let mut next = sizes.to_vec();
    next[index] = first;
    next[index + 1] = pair - first;
    next
}

/// The name of the program in the foreground of a terminal, e.g. `zsh` or `claude`.
fn process_name(pid: i32) -> Option<String> {
    let output = std::process::Command::new("ps").args(["-o", "comm=", "-p", &pid.to_string()]).output().ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let name = name.rsplit('/').next().unwrap_or(&name).to_owned();
    (!name.is_empty()).then_some(name)
}

impl Workspace {
    pub fn new(project: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Coming back to the window counts as looking at the pane in front.
        cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active()
                && let Some(front) = this.layout.focused_tab()
                && this.unseen.remove(&front)
            {
                cx.notify();
            }
        })
        .detach();
        let (bridge, calls) = start_bridge().unzip();
        if let Some(calls) = calls {
            cx.spawn(async move |this, cx| {
                while let Ok(call) = calls.recv().await {
                    if this.update(cx, |this, cx| this.handle_hook(call, cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        Self {
            project,
            layout: Layout::default(),
            terminals: HashMap::new(),
            names: HashMap::new(),
            unseen: HashSet::new(),
            renaming: None,
            next_tab: 0,
            focus: cx.focus_handle(),
            sidebar_open: true,
            editor_open: true,
            settings: settings::load(),
            bridge,
            session_names: HashMap::new(),
            badge: 0,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn change_settings(&mut self, change: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        change(&mut self.settings);
        settings::save(&self.settings);
        cx.notify();
    }

    /// A Claude Code hook from a session in one of the panes.
    fn handle_hook(&mut self, call: HookCall, cx: &mut Context<Self>) {
        let Some(tab) = call.pane.parse().ok().map(TabId) else { return call.done() };
        let Some(view) = self.terminals.get(&tab).cloned() else { return call.done() };
        let state = match call.event() {
            "UserPromptSubmit" | "PostToolUse" => Some(AgentState::Working),
            "PermissionRequest" => Some(AgentState::NeedsYou),
            "Stop" => Some(AgentState::YourTurn),
            "StopFailure" => Some(AgentState::Error),
            "SessionEnd" => Some(AgentState::Shell),
            "Notification" if call.field("notification_type") == Some("idle_prompt") => Some(AgentState::YourTurn),
            _ => None,
        };
        if let Some(state) = state {
            view.update(cx, |view, cx| view.hook_state(state, cx));
        }
        if call.event() == "CwdChanged"
            && let Some(cwd) = call.field("new_cwd").map(PathBuf::from)
        {
            view.update(cx, |view, _| view.cwd = cwd);
        }
        if call.event() == "UserPromptSubmit" && self.settings.rename_sessions_from_commands {
            let current = call.field("session_title");
            if let Some(name) = rename_for_prompt(
                call.field("prompt").unwrap_or_default(),
                current,
                self.session_names.get(&tab).map(String::as_str),
            ) {
                self.session_names.insert(tab, name.clone());
                return call.answer(&serde_json::json!({
                    "hookSpecificOutput": { "hookEventName": "UserPromptSubmit", "sessionTitle": name }
                }));
            }
        }
        call.done();
    }

    /// Starts a shell, in the focused terminal's folder or the project's, and places its tab.
    pub fn open_terminal(
        &mut self,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<TabId> {
        let cwd = self
            .layout
            .focused_tab()
            .and_then(|focused| self.terminals.get(&focused))
            .map_or_else(|| self.project.clone(), |view| view.read(cx).cwd.clone());
        self.next_tab += 1;
        let tab = TabId(self.next_tab);
        let mut env: Vec<(String, String)> = vec![
            ("TERM".into(), "xterm-256color".into()),
            ("COLORTERM".into(), "truecolor".into()),
            ("TERM_PROGRAM".into(), "Saturn".into()),
            ("SATURN_PANE_ID".into(), tab.0.to_string()),
        ];
        // Claude sessions in this pane load the Saturn plugin and report to this window only.
        if let (Some(bridge), Some(plugin)) = (&self.bridge, plugin_dir()) {
            env.push(("SATURN_SOCKET".into(), bridge.path().display().to_string()));
            let plugin = plugin.display().to_string();
            let dirs =
                std::env::var("CLAUDE_CODE_PLUGIN_DIRS").map_or(plugin.clone(), |dirs| format!("{dirs}:{plugin}"));
            env.push(("CLAUDE_CODE_PLUGIN_DIRS".into(), dirs));
        }
        let env: Vec<(&str, &str)> = env.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
        let (terminal, output) = match TerminalView::spawn(&cwd, &env) {
            Ok(started) => started,
            Err(error) => {
                eprintln!("saturn: could not start a shell in {}: {error:#}", cwd.display());
                return None;
            }
        };
        let view = cx.new(|cx| TerminalView::new(terminal, output, &cwd, cx));
        cx.subscribe_in(&view, window, move |this, _view, event, window, cx| {
            this.terminal_event(tab, event, window, cx);
        })
        .detach();
        self.terminals.insert(tab, view);
        match placement {
            Placement::Tab => self.layout.add_tab(tab, None),
            Placement::Right => self.layout.split_right(tab, None),
            Placement::Down => self.layout.split_down(tab, None),
        }
        self.after_layout_change(window, cx);
        Some(tab)
    }

    /// Opens a terminal next to a given cell (from its header buttons).
    fn open_in_cell(&mut self, cell: CellId, placement: Placement, window: &mut Window, cx: &mut Context<Self>) {
        self.layout.focus_cell(cell);
        self.open_terminal(placement, window, cx);
    }

    pub fn terminal(&self, tab: TabId) -> Option<&Entity<TerminalView>> {
        self.terminals.get(&tab)
    }

    fn is_looking_at(&self, tab: TabId, window: &Window) -> bool {
        window.is_window_active() && self.layout.focused_tab() == Some(tab)
    }

    fn terminal_event(&mut self, tab: TabId, event: &TerminalViewEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            TerminalViewEvent::Exited => self.forget(tab, window, cx),
            TerminalViewEvent::StateChanged => {
                let asks = self.terminals.get(&tab).is_some_and(|view| view.read(cx).state.asks_for_you());
                if asks && !self.is_looking_at(tab, window) {
                    self.unseen.insert(tab);
                } else if !asks {
                    self.unseen.remove(&tab);
                }
                cx.notify();
            }
            TerminalViewEvent::Bell => {
                if !self.is_looking_at(tab, window) {
                    self.unseen.insert(tab);
                }
                cx.notify();
            }
            TerminalViewEvent::TitleChanged => cx.notify(),
        }
    }

    fn forget(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.terminals.remove(&tab);
        self.unseen.remove(&tab);
        self.names.remove(&tab);
        self.layout.remove_tab(tab);
        self.after_layout_change(window, cx);
    }

    /// Focuses the terminal now in front and marks it seen.
    fn after_layout_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(front) = self.layout.focused_tab() {
            self.unseen.remove(&front);
            if self.renaming.is_none()
                && let Some(view) = self.terminals.get(&front)
            {
                window.focus(&view.focus_handle(cx), cx);
            }
        } else {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn update_layout(&mut self, window: &mut Window, cx: &mut Context<Self>, change: impl FnOnce(&mut Layout)) {
        change(&mut self.layout);
        self.after_layout_change(window, cx);
    }

    /// Closes a tab, asking first when something other than the shell runs in it. The tab goes
    /// when its shell reports the exit.
    fn close_tab(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.terminals.get(&tab).cloned() else { return };
        let running =
            view.read(cx).foreground_pid().and_then(process_name).filter(|name| !SHELLS.contains(&name.as_str()));
        let Some(program) = running else {
            view.update(cx, |view, _| view.kill());
            return;
        };
        let message = format!("Close {}?", self.tab_name(tab, cx));
        let detail = format!("{program} is still running in it and will be stopped.");
        let answer = window.prompt(PromptLevel::Warning, &message, Some(&detail), &["Close", "Cancel"], cx);
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                view.update(cx, |view, _| view.kill());
            }
        })
        .detach();
    }

    /// A name typed by hand, else (when the setting is on) the session's or terminal's title,
    /// else the folder the terminal is in.
    fn tab_name(&self, tab: TabId, cx: &Context<Self>) -> String {
        if let Some(name) = self.names.get(&tab) {
            return name.clone();
        }
        let Some(view) = self.terminals.get(&tab).map(|view| view.read(cx)) else { return "zsh".to_owned() };
        let title = self.settings.name_tabs_after_sessions.then(|| view.title.as_deref().and_then(tab_title)).flatten();
        title.unwrap_or_else(|| {
            view.cwd.file_name().map_or_else(|| "zsh".to_owned(), |folder| folder.to_string_lossy().into_owned())
        })
    }

    fn tab_state(&self, tab: TabId, cx: &Context<Self>) -> AgentState {
        self.terminals.get(&tab).map_or(AgentState::Shell, |view| view.read(cx).state)
    }

    fn start_rename(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.tab_name(tab, cx);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        cx.subscribe_in(&input, window, move |this, input, event, window, cx| match event {
            InputEvent::PressEnter { .. } | InputEvent::Blur => {
                let name = input.read(cx).value().trim().to_owned();
                this.finish_rename(tab, Some(name), window, cx);
            }
            _ => {}
        })
        .detach();
        input.update(cx, |input, cx| input.focus(window, cx));
        self.renaming = Some((tab, input));
        cx.notify();
    }

    /// Saves (or with `None`, drops) a typed name; an empty name goes back to the terminal title.
    fn finish_rename(&mut self, tab: TabId, name: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.as_ref().is_none_or(|(renaming, _)| *renaming != tab) {
            return;
        }
        self.renaming = None;
        match name {
            Some(name) if !name.is_empty() => {
                self.names.insert(tab, name);
            }
            Some(_) => {
                self.names.remove(&tab);
            }
            None => {}
        }
        self.after_layout_change(window, cx);
    }

    fn drop_tab(
        &mut self,
        tab: TabId,
        cell: CellId,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update_layout(window, cx, |layout| layout.move_tab(tab, cell, index));
    }

    fn open_project(window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Project".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            let Some(project) = paths.into_iter().next() else { return };
            settings::remember_project(&project);
            let _ = this.update_in(cx, |this, window, cx| {
                this.project = project;
                if this.layout.is_empty() {
                    this.open_terminal(Placement::Tab, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_tab(&self, cell: &Cell, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let tab = cell.tabs[index];
        let cell_id = cell.id;
        let selected = tab == cell.active;
        let state = self.tab_state(tab, cx);
        let name = self.tab_name(tab, cx);
        // A tab hidden behind another that needs you turns yellow, so it can't be missed.
        let calling = !selected && state == AgentState::NeedsYou && self.unseen.contains(&tab);
        let (background, foreground) = match (calling, selected) {
            (true, _) => (theme::color(color::YELLOW), theme::color(color::INK_INVERSE)),
            (false, true) => (theme::color(color::CONTROL_PRESSED), theme::color(color::INK)),
            (false, false) => (gpui_kit::transparent_black(), theme::color(color::INK_MUTED)),
        };
        let renaming = self.renaming.as_ref().filter(|(renaming, _)| *renaming == tab).map(|(_, input)| input.clone());

        let close = div()
            .id(("close", tab.0))
            .flex()
            .items_center()
            .justify_center()
            .size(px(18.))
            .rounded_full()
            .when(!selected, |close| close.invisible().group_hover("tab", gpui_kit::Styled::visible))
            .hover(|s| s.bg(theme::color(color::CONTROL_HOVER)))
            .child(chrome::icon("x", 11., foreground))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, window, cx| this.close_tab(tab, window, cx)));

        div()
            .id(("tab", tab.0))
            .group("tab")
            .flex()
            .flex_none()
            .items_center()
            .gap(len(6.))
            .h(len(TAB_HEIGHT))
            .pl(len(space::SPACE_3))
            .pr(len(space::SPACE_1))
            .max_w(px(220.))
            .rounded(len(radius::RADIUS_LG))
            .bg(background)
            .when(selected && !calling, |tab| tab.shadow(theme::shadows(shadow::LIFT)))
            .text_color(foreground)
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(status_dot(state))
            .map(|tab_element| match renaming {
                Some(input) => tab_element.child(div().w(px(140.)).child(Input::new(&input).appearance(false))),
                None => tab_element.child(div().truncate().child(name.clone())),
            })
            .child(close)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.update_layout(window, cx, |layout| layout.activate_tab(tab));
                }),
            )
            .on_mouse_down(MouseButton::Middle, cx.listener(move |this, _, window, cx| this.close_tab(tab, window, cx)))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() >= 2 {
                    this.start_rename(tab, window, cx);
                }
            }))
            .on_drag(DraggedTab { tab, name }, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .drag_over::<DraggedTab>(|style, _, _, _| style.border_l_2().border_color(theme::color(color::PURPLE_TEXT)))
            .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                this.drop_tab(dragged.tab, cell_id, Some(index), window, cx);
            }))
            .into_any_element()
    }

    fn render_cell(&self, cell: &Cell, cx: &mut Context<Self>) -> AnyElement {
        let focused = self.layout.focused == Some(cell.id);
        let unseen_states: Vec<AgentState> =
            cell.tabs.iter().filter(|tab| self.unseen.contains(tab)).map(|tab| self.tab_state(*tab, cx)).collect();
        let needs_you = unseen_states.contains(&AgentState::NeedsYou);
        let glow = if needs_you {
            Some(shadow::GLOW_NEEDS_YOU)
        } else if !unseen_states.is_empty() {
            Some(shadow::GLOW_YOUR_TURN)
        } else if focused {
            Some(shadow::GLOW_FOCUS)
        } else {
            None
        };
        let terminal = self.terminals.get(&cell.active).cloned();
        let cell_id: CellId = cell.id;
        let header_button = |id: &'static str, icon: &str, placement: Placement, cx: &mut Context<Self>| {
            chrome::icon_tile((id, cell_id.key()), icon, TileKind::Round, TAB_HEIGHT)
                .on_click(cx.listener(move |this, _, window, cx| this.open_in_cell(cell_id, placement, window, cx)))
        };

        let mut pane = div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .rounded(len(radius::RADIUS_XL))
            .bg(theme::color(color::SURFACE))
            .border_1()
            .border_color(theme::color(color::LINE))
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.update_layout(window, cx, |layout| layout.focus_cell(cell_id));
                }),
            )
            .child(
                div()
                    .id(("tabs", cell_id.key()))
                    .flex()
                    .items_center()
                    .gap(len(space::SPACE_1))
                    .h(len(PANE_HEADER_HEIGHT))
                    .p(len(space::SPACE_2))
                    // GPUI clips children to a rectangle, not to the pane's rounded corners, so the
                    // header rounds its own: the pane's radius minus its one-pixel border.
                    .rounded_t(len(radius::RADIUS_XL - 1.))
                    .bg(theme::color(color::SURFACE_RAISED))
                    .shadow(theme::shadows(shadow::LIFT))
                    .child(
                        div()
                            .id(("tab-strip", cell_id.key()))
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .overflow_x_scroll()
                            .children((0..cell.tabs.len()).map(|index| self.render_tab(cell, index, cx)))
                            .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                                this.drop_tab(dragged.tab, cell_id, None, window, cx);
                            })),
                    )
                    .child(header_button("split-right", "columns-2", Placement::Right, cx))
                    .child(header_button("split-down", "rows-2", Placement::Down, cx))
                    .child(header_button("new-tab", "plus", Placement::Tab, cx)),
            )
            .child(
                div()
                    .id(("body", cell_id.key()))
                    .flex_1()
                    .min_h_0()
                    .pt(len(space::SPACE_3))
                    .pl(len(space::SPACE_4))
                    .pb(len(space::SPACE_1))
                    .drag_over::<DraggedTab>(|style, _, _, _| style.bg(theme::color(color::PURPLE).opacity(0.08)))
                    .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                        this.drop_tab(dragged.tab, cell_id, None, window, cx);
                    }))
                    .children(terminal),
            );
        if let Some(glow) = glow {
            pane = pane.border_color(gpui_kit::transparent_black()).shadow(theme::shadows(glow));
        }
        if needs_you {
            // Needs you breathes, slowly, so it is seen from the corner of the eye without flashing.
            return pane
                .with_animation(
                    ("breathe", cell_id.key()),
                    Animation::new(Duration::from_millis(2400)).repeat().with_easing(bounce(ease_in_out)),
                    |pane, t| pane.shadow(theme::shadows(&breathing_glow(t))),
                )
                .into_any_element();
        }
        pane.into_any_element()
    }

    /// A divider between rows or cells: the grid's gap, draggable.
    fn render_divider(divider: GridDivider, vertical: bool) -> AnyElement {
        let id = match divider {
            GridDivider::Rows(index) => ("row-divider", index as u64),
            GridDivider::Cells(row, index) => ("cell-divider", row.key() * 1000 + index as u64),
        };
        div()
            .id(id)
            .flex_none()
            .when(vertical, |d| d.w(len(space::SPACE_3)).h_full().cursor(CursorStyle::ResizeLeftRight))
            .when(!vertical, |d| d.h(len(space::SPACE_3)).w_full().cursor(CursorStyle::ResizeUpDown))
            .on_drag(divider, |_, _, _, cx| cx.new(|_| DividerPreview))
            .into_any_element()
    }

    fn render_grid(&self, cx: &mut Context<Self>) -> AnyElement {
        let maximized = self.layout.maximized;
        let row_sizes: Vec<f32> = self.layout.rows.iter().map(|row| row.size).collect();
        let mut grid = div().id("grid").flex().flex_col().flex_1().min_h_0().on_drag_move(cx.listener(
            move |this, event: &DragMoveEvent<GridDivider>, _, cx| {
                if let GridDivider::Rows(index) = *event.drag(cx) {
                    let at =
                        f32::from(event.event.position.y - event.bounds.top()) / f32::from(event.bounds.size.height);
                    let sizes = move_divider(&row_sizes, index, at);
                    this.layout.resize_rows(&sizes);
                    cx.notify();
                }
            },
        ));
        for (row_index, row) in self.layout.rows.iter().enumerate() {
            let shown: Vec<&Cell> = row.cells.iter().filter(|cell| maximized.is_none_or(|id| id == cell.id)).collect();
            if shown.is_empty() {
                continue;
            }
            if row_index > 0 && maximized.is_none() {
                grid = grid.child(Self::render_divider(GridDivider::Rows(row_index - 1), false));
            }
            let row_id = row.id;
            let cell_sizes: Vec<f32> = row.cells.iter().map(|cell| cell.size).collect();
            let mut row_element =
                div().id(("row", row_id.key())).flex().min_h_0().flex_basis(relative(0.)).on_drag_move(cx.listener(
                    move |this, event: &DragMoveEvent<GridDivider>, _, cx| {
                        if let GridDivider::Cells(dragged_row, index) = *event.drag(cx)
                            && dragged_row == row_id
                        {
                            let at = f32::from(event.event.position.x - event.bounds.left())
                                / f32::from(event.bounds.size.width);
                            let sizes = move_divider(&cell_sizes, index, at);
                            this.layout.resize_cells(row_id, &sizes);
                            cx.notify();
                        }
                    },
                ));
            row_element.style().flex_grow = Some(if maximized.is_some() { 1.0 } else { row.size });
            for (cell_index, cell) in shown.into_iter().enumerate() {
                if cell_index > 0 && maximized.is_none() {
                    row_element =
                        row_element.child(Self::render_divider(GridDivider::Cells(row_id, cell_index - 1), true));
                }
                let mut cell_element =
                    div().flex().min_w_0().flex_basis(relative(0.)).child(self.render_cell(cell, cx));
                cell_element.style().flex_grow = Some(if maximized.is_some() { 1.0 } else { cell.size });
                row_element = row_element.child(cell_element);
            }
            grid = grid.child(row_element);
        }
        grid.into_any_element()
    }

    fn render_titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let states: Vec<AgentState> = self.terminals.values().map(|view| view.read(cx).state).collect();
        let count = |wanted: &[AgentState]| states.iter().filter(|state| wanted.contains(state)).count();
        let summary = [
            (count(&[AgentState::NeedsYou]), AgentState::NeedsYou, "need you"),
            (count(&[AgentState::YourTurn, AgentState::Error]), AgentState::YourTurn, "your turn"),
            (count(&[AgentState::Working]), AgentState::Working, "working"),
        ];
        let toggle = |id: &'static str, icon: &str, on: bool| {
            chrome::icon_tile(id, icon, if on { TileKind::Plain } else { TileKind::Off }, 28.)
        };
        // The title bar is drawn by Saturn, so the empty parts of it pass a double-click on to
        // macOS, which zooms or minimizes as System Settings says.
        let empty = |id: &'static str| {
            div().id(id).h_full().on_click(|event: &ClickEvent, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
        };
        div()
            .flex()
            .items_center()
            .h(len(TITLEBAR_HEIGHT))
            .pr(len(space::SPACE_1))
            .text_size(px(12.))
            .child(empty("titlebar-lights").flex_none().w(len(TRAFFIC_LIGHTS_WIDTH)))
            .child(
                div()
                    .flex()
                    .gap(len(space::SPACE_2))
                    .child(toggle("toggle-sidebar", "panel-left", self.sidebar_open).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.sidebar_open = !this.sidebar_open;
                            cx.notify();
                        },
                    )))
                    .child(toggle("toggle-editor", "panel-top", self.editor_open).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.editor_open = !this.editor_open;
                            cx.notify();
                        },
                    ))),
            )
            .child(empty("titlebar-middle").flex_1())
            .children(summary.into_iter().filter(|(n, _, _)| *n > 0).map(|(n, state, label)| {
                div()
                    .id(label)
                    .flex()
                    .items_center()
                    .ml(len(space::SPACE_5))
                    .gap(len(6.))
                    .cursor_pointer()
                    .text_color(if state == AgentState::Working {
                        theme::color(color::INK_MUTED)
                    } else {
                        state_color(state)
                    })
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(status_dot(state))
                    .child(format!("{n} {label}"))
                    .on_click(cx.listener(|this, _, window, cx| this.next_attention(window, cx)))
            }))
            .child(
                // The empty grid has its own primary button; one per screen.
                chrome::button("new-terminal", "New terminal", Some("⌘T"), !self.layout.is_empty())
                    .ml(len(space::SPACE_5))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_terminal(Placement::Tab, window, cx);
                    })),
            )
    }

    /// Sidebar | (editor / grid), with draggable dividers. The panels stay in the same place in
    /// the tree when toggled, so terminals never restart.
    fn render_workbench(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let half_gap = len(space::SPACE_3 / 2.);
        let name = self.project.file_name().map_or_else(|| "/".to_owned(), |name| name.to_string_lossy().into_owned());
        let grid = if self.layout.is_empty() {
            chrome::empty_grid(
                &name,
                cx.listener(|this, _, window, cx| {
                    this.open_terminal(Placement::Tab, window, cx);
                }),
            )
        } else {
            self.render_grid(cx)
        };
        let path = self.project.display().to_string();
        div().flex_1().min_h_0().child(
            h_resizable("workbench")
                .with_handle_appearance(chrome::quiet_resize_handle())
                .child(
                    resizable_panel()
                        .size(px(270.))
                        .size_range(px(200.)..px(480.))
                        .visible(self.sidebar_open)
                        .child(div().size_full().pr(half_gap).child(chrome::sidebar(&name, &path))),
                )
                .child(
                    resizable_panel().child(
                        div().size_full().when(self.sidebar_open, |main| main.pl(half_gap)).child(
                            v_resizable("main")
                                .with_handle_appearance(chrome::quiet_resize_handle())
                                .child(
                                    resizable_panel()
                                        .size(px(260.))
                                        .size_range(px(120.)..px(4000.))
                                        .visible(self.editor_open)
                                        .child(div().size_full().pb(half_gap).child(chrome::editor())),
                                )
                                .child(
                                    resizable_panel().child(
                                        div()
                                            .size_full()
                                            .flex()
                                            .flex_col()
                                            .when(self.editor_open, |grid| grid.pt(half_gap))
                                            .child(grid),
                                    ),
                                ),
                        ),
                    ),
                ),
        )
    }

    fn next_attention(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let order: Vec<TabId> = self.layout.tabs().collect();
        let start =
            self.layout.focused_tab().and_then(|front| order.iter().position(|t| *t == front)).map_or(0, |i| i + 1);
        let target = order
            .iter()
            .cycle()
            .skip(start)
            .take(order.len())
            .find(|tab| self.tab_state(**tab, cx).asks_for_you())
            .copied();
        if let Some(tab) = target {
            self.update_layout(window, cx, |layout| layout.activate_tab(tab));
        }
    }

    fn focused_view(&self) -> Option<Entity<TerminalView>> {
        self.layout.focused_tab().and_then(|tab| self.terminals.get(&tab)).cloned()
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.unseen.len() != self.badge {
            self.badge = self.unseen.len();
            dock::set_badge(self.badge);
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .px(len(space::SPACE_3))
            .pb(len(space::SPACE_3))
            .bg(theme::color(color::GROUND))
            .text_color(theme::color(color::INK))
            .font_family(saturn_design::font::SANS)
            .track_focus(&self.focus)
            .key_context("Workspace")
            .on_action(cx.listener(|this, _: &NewTab, window, cx| {
                this.open_terminal(Placement::Tab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SplitRight, window, cx| {
                this.open_terminal(Placement::Right, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SplitDown, window, cx| {
                this.open_terminal(Placement::Down, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if let Some(tab) = this.layout.focused_tab() {
                    this.close_tab(tab, window, cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &NextTab, window, cx| this.update_layout(window, cx, |l| l.cycle_tab(true))),
            )
            .on_action(
                cx.listener(|this, _: &PreviousTab, window, cx| this.update_layout(window, cx, |l| l.cycle_tab(false))),
            )
            .on_action(cx.listener(|this, _: &FocusLeft, window, cx| {
                this.update_layout(window, cx, |l| l.focus_direction(Direction::Left));
            }))
            .on_action(cx.listener(|this, _: &FocusRight, window, cx| {
                this.update_layout(window, cx, |l| l.focus_direction(Direction::Right));
            }))
            .on_action(cx.listener(|this, _: &FocusUp, window, cx| {
                this.update_layout(window, cx, |l| l.focus_direction(Direction::Up));
            }))
            .on_action(cx.listener(|this, _: &FocusDown, window, cx| {
                this.update_layout(window, cx, |l| l.focus_direction(Direction::Down));
            }))
            .on_action(cx.listener(|this, _: &ToggleMaximize, window, cx| {
                this.update_layout(window, cx, Layout::toggle_maximize);
            }))
            .on_action(cx.listener(|this, _: &NextAttention, window, cx| this.next_attention(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar_open = !this.sidebar_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleEditor, _, cx| {
                this.editor_open = !this.editor_open;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &OpenProject, window, cx| Self::open_project(window, cx)))
            .on_action(cx.listener(|_, _: &OpenSettings, window, cx| settings_view::open(cx.weak_entity(), window, cx)))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if let Some(view) = this.focused_view() {
                    view.update(cx, |view, cx| view.copy_selection(cx));
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                if let Some(view) = this.focused_view() {
                    view.update(cx, TerminalView::paste_from_clipboard);
                }
            }))
            .on_action(
                cx.listener(|this, _: &FocusPane1, window, cx| this.update_layout(window, cx, |l| l.focus_nth(1))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane2, window, cx| this.update_layout(window, cx, |l| l.focus_nth(2))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane3, window, cx| this.update_layout(window, cx, |l| l.focus_nth(3))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane4, window, cx| this.update_layout(window, cx, |l| l.focus_nth(4))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane5, window, cx| this.update_layout(window, cx, |l| l.focus_nth(5))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane6, window, cx| this.update_layout(window, cx, |l| l.focus_nth(6))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane7, window, cx| this.update_layout(window, cx, |l| l.focus_nth(7))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane8, window, cx| this.update_layout(window, cx, |l| l.focus_nth(8))),
            )
            .on_action(
                cx.listener(|this, _: &FocusPane9, window, cx| this.update_layout(window, cx, |l| l.focus_nth(9))),
            )
            .child(self.render_titlebar(cx))
            .child(self.render_workbench(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::move_divider;

    #[test]
    fn moves_a_divider_and_keeps_the_pair_total() {
        let sizes = move_divider(&[1.0, 1.0, 2.0], 0, 0.125);
        assert!((sizes[0] - 0.5).abs() < 1e-6 && (sizes[1] - 1.5).abs() < 1e-6 && (sizes[2] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn keeps_both_neighbours_above_the_minimum() {
        let sizes = move_divider(&[1.0, 1.0], 0, 0.0);
        assert!((sizes[0] - 2.0 * 0.08).abs() < 1e-6);
        let sizes = move_divider(&[1.0, 1.0], 0, 1.0);
        assert!((sizes[1] - 2.0 * 0.08).abs() < 1e-6);
    }
}
