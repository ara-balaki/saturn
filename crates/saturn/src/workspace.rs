//! The window's root: the title bar and the grid of terminal panes, each with tabs.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, AppContext as _, Bounds, ClickEvent, Context, CursorStyle, DragMoveEvent,
    Entity, FocusHandle, Focusable as _, FontWeight, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, PathPromptOptions, Pixels, Point, PromptLevel, Render, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, actions, bounce, div, ease_in_out, prelude::FluentBuilder as _, px, relative,
};
use saturn_bridge::{Bridge, HookCall};
use saturn_core::agent_activity;
use saturn_core::agent_state::{AgentState, tab_title};
use saturn_core::clickup::Ticket;
use saturn_core::git;
use saturn_core::layout::{Cell, CellId, Direction, Layout, RowId, TabId};
use saturn_core::pull_request::{PrState, PullRequest};
use saturn_core::session_name::rename_for_prompt;
use saturn_design::{color, radius, shadow, space};
use saturn_terminal::view::{TerminalView, TerminalViewEvent};

use crate::changes_view::{ChangesEvent, ChangesView};
use crate::chrome;
use crate::clickup;
use crate::editor_view::EditorView;
use crate::files_view::{FilesEvent, FilesView};
use crate::session::{self, SavedGrid, SavedTab};
use crate::settings::{self, AppearanceSetting, Settings};
use crate::status_bar::StatusBar;
use crate::theme::{self, len};
use crate::{agents_view, dock, github, menu_bar, notify, palette, settings_view, ticket_picker};

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
        SaveFile,
        OpenTicketPicker,
        FocusPane1,
        FocusPane2,
        FocusPane3,
        FocusPane4,
        FocusPane5,
        FocusPane6,
        FocusPane7,
        FocusPane8,
        FocusPane9,
        GoToFile,
        CommandPalette,
        ShowHistory,
        ShowImpact,
        ShowSchema,
        ShowProblems,
    ]
);

thread_local! {
    /// Every window's workspace, so app-wide shortcuts can find the one in front.
    static OPEN: std::cell::RefCell<Vec<(gpui_kit::AnyWindowHandle, gpui_kit::WeakEntity<Workspace>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs `act` on the workspace of the window in front, if it has one: for shortcuts that arrive
/// while nothing in the window has the keyboard.
pub fn in_front(cx: &mut gpui_kit::App, act: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>)) {
    let Some(front) = cx.active_window() else { return };
    let workspace = OPEN.with_borrow_mut(|open| {
        open.retain(|(_, workspace)| workspace.upgrade().is_some());
        open.iter().find(|(window, _)| *window == front).map(|(_, workspace)| workspace.clone())
    });
    if let Some(workspace) = workspace.and_then(|workspace| workspace.upgrade()) {
        let _ = front.update(cx, |_, window, cx| workspace.update(cx, |workspace, cx| act(workspace, window, cx)));
    }
}

/// Every command the command palette offers: its name, its icon, and the action it runs (whose
/// shortcut the palette shows).
pub fn commands() -> Vec<(&'static str, &'static str, Box<dyn gpui_kit::Action>)> {
    vec![
        ("Go to file", "search", Box::new(GoToFile)),
        ("New terminal", "plus", Box::new(NewTab)),
        ("Split right", "columns-2", Box::new(SplitRight)),
        ("Split down", "rows-2", Box::new(SplitDown)),
        ("Close terminal", "x", Box::new(CloseTab)),
        ("Next tab", "square-arrow-right", Box::new(NextTab)),
        ("Previous tab", "arrow-left", Box::new(PreviousTab)),
        ("Maximise the pane", "maximize-2", Box::new(ToggleMaximize)),
        ("Go to the next agent that needs you", "bot", Box::new(NextAttention)),
        ("New agent from a ticket", "square-plus", Box::new(OpenTicketPicker)),
        ("Show or hide the sidebar", "panel-left", Box::new(ToggleSidebar)),
        ("Show or hide the editor", "panel-top", Box::new(ToggleEditor)),
        ("History", "git-branch", Box::new(ShowHistory)),
        ("Impact of the changes", "waypoints", Box::new(ShowImpact)),
        ("Database diagram", "database", Box::new(ShowSchema)),
        ("Problems", "circle-x", Box::new(ShowProblems)),
        ("Save the file", "file-code", Box::new(SaveFile)),
        ("Add a repo", "folder", Box::new(OpenProject)),
        ("Settings", "settings", Box::new(OpenSettings)),
        ("Check for updates", "square-arrow-right", Box::new(crate::CheckForUpdates)),
    ]
}

#[derive(Clone, Copy)]
pub enum Placement {
    Tab,
    Right,
    Down,
}

/// Tall enough that the title bar's outer controls sit 10px from the window's edges: 48 = 28 + 2 × 10.
const TITLEBAR_HEIGHT: f32 = 48.0;
const TRAFFIC_LIGHTS_WIDTH: f32 = 76.0;
const TAB_HEIGHT: f32 = chrome::CONTROL_HEIGHT;
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
            .rounded(len(radius::RADIUS_SM))
            .bg(theme::fill_pressed())
            .border_1()
            .border_color(theme::color(color::BLUE))
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
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
    /// The repos this window holds, in the sidebar's order. The first names the saved session.
    projects: Vec<PathBuf>,
    /// The selected repo: where new tabs and ticket worktrees go, and whose ticket settings apply.
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
    /// Agents waiting for you, as last told to the menu bar.
    waiting: usize,
    /// The sidebar's tree and changes, per repository or worktree it has shown, so switching back
    /// is instant and each keeps watching its folder.
    roots: HashMap<PathBuf, RootViews>,
    /// The folder the sidebar shows: the project, or the focused pane's worktree.
    root: PathBuf,
    /// Whether the sidebar follows the focused pane's worktree.
    follow_pane: bool,
    /// Open files, in the top panel.
    editor: Entity<EditorView>,
    /// Along the window's foot: problems, the language server, the cursor, the branch.
    status_bar: Entity<StatusBar>,
    /// What the sidebar shows.
    sidebar_tab: SidebarTab,
    /// The Claude session running in each tab, from its hooks, so a restart can offer to resume it.
    claude_sessions: HashMap<TabId, String>,
    /// ClickUp, when a token is set; nothing about it shows otherwise.
    clickup: ClickUpState,
    /// Each agent's steps and context, for the Agents tab.
    activity: HashMap<TabId, Activity>,
    /// Where a dragged tab would land: a cell, and the side to split it on (`None`: into the cell).
    drop_target: Option<(CellId, Option<Direction>)>,
    /// The repository and branch each tab is in, read in the background.
    checkouts: HashMap<TabId, Checkout>,
    /// The pull request of each branch a tab is on, from `gh`.
    pull_requests: HashMap<Checkout, PullRequest>,
    /// Whether the two above are being read, so refreshes don't pile up.
    checkout_refresh: Refresh,
}

/// A background refresh: not running, running, or running with another asked for meanwhile (a new
/// tab, a folder change), to run once it's done.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Refresh {
    Idle,
    Running,
    RunningStale,
}

/// A branch checked out in a repository (or worktree), named by its top folder.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Checkout {
    root: PathBuf,
    branch: String,
}

/// The ClickUp connection as the window knows it.
#[derive(Default)]
pub struct ClickUpState {
    token: Option<String>,
    /// The workspace the token reaches, once checked.
    pub account: Option<String>,
    /// The last thing that went wrong, shown in Settings and the ticket picker.
    pub problem: Option<String>,
    /// Open tickets assigned to the token's owner, refreshed every minute.
    pub tickets: Vec<Ticket>,
    pub loading: bool,
}

impl ClickUpState {
    pub fn connected(&self) -> bool {
        self.token.is_some()
    }
}

/// What the sidebar shows for one folder.
struct RootViews {
    files: Entity<FilesView>,
    changes: Entity<ChangesView>,
    /// The branch checked out there, for the sidebar's header.
    branch: Option<String>,
    /// Whether it describes a database (Prisma or SQL migrations), for the diagram's button.
    has_schema: bool,
    _events: [Subscription; 2],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarTab {
    Files,
    Changes,
}

/// What an agent is up to beyond its state, from its hooks and transcript, for the Agents tab.
#[derive(Default)]
struct Activity {
    steps: Option<agent_activity::Steps>,
    /// Tokens in the session's context after its latest reply.
    context: Option<u64>,
}

/// The Saturn plugin that Claude sessions in Saturn's panes load: inside Saturn.app
/// (`Contents/Resources/plugin`), else `plugin/` in this repository when run from source.
fn plugin_dir() -> Option<PathBuf> {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|macos| macos.join("../Resources/plugin")))
        .and_then(|dir| dir.canonicalize().ok());
    std::env::var_os("SATURN_PLUGIN_DIR")
        .map(PathBuf::from)
        .or(bundled)
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

/// A pane's ticket: its status in ClickUp's own colour, opening the ticket.
fn ticket_chip(cell: CellId, ticket: &Ticket) -> impl IntoElement {
    let url = ticket.url.clone();
    div()
        .id(("ticket-chip", cell.key()))
        .flex()
        .flex_none()
        .items_center()
        .gap(len(space::SPACE_2))
        .h(len(TAB_HEIGHT))
        .px(len(space::SPACE_3))
        .rounded(len(radius::RADIUS_SM))
        .cursor_pointer()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::color(color::INK_MUTED))
        .hover(|s| s.bg(theme::fill_hover()))
        .child(ticket_picker::status_dot(ticket))
        .child(ticket.status.clone())
        .on_click(move |_, _, _| {
            let _ = std::process::Command::new("open").arg(&url).spawn();
        })
}

/// A pane's pull request: its number in the colour of its state, opening it on GitHub.
fn pull_request_chip(cell: CellId, pr: &PullRequest) -> impl IntoElement {
    let (icon, state_color) = match pr.state {
        PrState::Open => ("git-pull-request", color::TERM_GREEN),
        PrState::Draft => ("git-pull-request-draft", color::INK_FAINT),
        PrState::Merged => ("git-merge", color::TERM_MAGENTA),
        PrState::Closed => ("git-pull-request-closed", color::RED),
    };
    let url = pr.url.clone();
    let tip = format!("{} · {}", pr.state.label(), pr.title);
    div()
        .id(("pr-chip", cell.key()))
        .flex()
        .flex_none()
        .items_center()
        .gap(len(space::SPACE_1))
        .h(len(TAB_HEIGHT))
        .px(len(space::SPACE_2))
        .rounded(len(radius::RADIUS_SM))
        .cursor_pointer()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::color(color::INK_MUTED))
        .hover(|s| s.bg(theme::fill_hover()))
        .child(chrome::icon(icon, 14., theme::color(state_color)))
        .child(format!("#{}", pr.number))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .on_click(move |_, _, _| {
            let _ = std::process::Command::new("open").arg(&url).spawn();
        })
}

/// Where in a pane a dragged tab lands: within a quarter of an edge splits the pane on that side,
/// anywhere further in moves the tab into the pane (`None`).
fn drop_side(bounds: Bounds<Pixels>, at: Point<Pixels>) -> Option<Direction> {
    let x = f32::from(at.x - bounds.origin.x) / f32::from(bounds.size.width).max(1.);
    let y = f32::from(at.y - bounds.origin.y) / f32::from(bounds.size.height).max(1.);
    let edges = [(x, Direction::Left), (1. - x, Direction::Right), (y, Direction::Up), (1. - y, Direction::Down)];
    let (distance, side) =
        edges.into_iter().fold((f32::MAX, Direction::Left), |best, edge| if edge.0 < best.0 { edge } else { best });
    (distance < 0.25).then_some(side)
}

/// The blue preview of where a dropped tab goes: half the pane on a side, or the whole pane.
fn drop_preview(side: Option<Direction>) -> impl IntoElement {
    let preview = div()
        .absolute()
        .rounded(len(radius::RADIUS_SM))
        .bg(theme::color(color::BLUE_VIVID).opacity(0.14))
        .border_2()
        .border_color(theme::color(color::BLUE).opacity(0.7));
    match side {
        Some(Direction::Left) => preview.top_0().left_0().h_full().w(relative(0.5)),
        Some(Direction::Right) => preview.top_0().right_0().h_full().w(relative(0.5)),
        Some(Direction::Up) => preview.top_0().left_0().w_full().h(relative(0.5)),
        Some(Direction::Down) => preview.bottom_0().left_0().w_full().h(relative(0.5)),
        None => preview.top_0().left_0().size_full(),
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
    let mut glow = shadow::GLOW_NEEDS_YOU.get().to_vec();
    if let Some(halo) = glow.get_mut(1) {
        halo.blur += 4. * t;
        halo.color.alpha += 0.12 * t;
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
        // A saved ClickUp token turns the ticket picker on; the Keychain is read off the main thread.
        cx.spawn(async move |this, cx| {
            let token = cx.background_spawn(async { clickup::token() }).await;
            if let Some(token) = token {
                let _ = this.update(cx, |this, cx| {
                    this.clickup.token = Some(token);
                    this.refresh_tickets(cx);
                });
            }
            // Statuses change in ClickUp; a minute is fresh enough and far under its rate limit.
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(60)).await;
                if this.update(cx, Workspace::refresh_tickets).is_err() {
                    break;
                }
            }
        })
        .detach();
        // Branches change inside panes without telling Saturn, and pull requests change on GitHub;
        // a minute is fresh enough and far under GitHub's rate limit.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(60)).await;
                if this.update(cx, Workspace::refresh_checkouts).is_err() {
                    break;
                }
            }
        })
        .detach();
        // Pane sizes change by dragging, which saves nothing on its own; quitting saves them.
        cx.on_app_quit(|this, cx| {
            this.save_session(cx);
            async {}
        })
        .detach();
        // Following the system: redraw when macOS switches between light and dark.
        cx.observe_window_appearance(window, |this, _, cx| {
            if this.settings.appearance == AppearanceSetting::System {
                theme::apply(AppearanceSetting::System, cx);
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
        let roots = HashMap::from([(project.clone(), Self::root_views(&project, window, cx))]);
        // The editor panel can take the grid's room; the workbench follows its changes.
        let editor = cx.new(|_| EditorView::default());
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        let handle = window.window_handle();
        let weak = cx.weak_entity();
        OPEN.with_borrow_mut(|open| open.push((handle, weak)));
        let status_bar = {
            let (editor, workspace) = (editor.clone(), cx.weak_entity());
            cx.new(|cx| StatusBar::new(editor, workspace, &project, cx))
        };
        Self {
            status_bar,
            sidebar_tab: SidebarTab::Files,
            claude_sessions: HashMap::new(),
            clickup: ClickUpState::default(),
            checkouts: HashMap::new(),
            activity: HashMap::new(),
            drop_target: None,
            pull_requests: HashMap::new(),
            checkout_refresh: Refresh::Idle,
            roots,
            root: project.clone(),
            follow_pane: true,
            editor,
            projects: vec![project.clone()],
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
            waiting: 0,
        }
    }

    /// Opens `path` in the editor, showing the editor if it was hidden.
    pub fn open_file(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| editor.open(path, window, cx));
        self.editor_open = true;
        cx.notify();
    }

    /// A tree and a changes list for `root`. A file picked in the tree opens in the editor; a new
    /// git state makes the changes list read again.
    fn root_views(root: &Path, window: &mut Window, cx: &mut Context<Self>) -> RootViews {
        let files = cx.new(|cx| FilesView::new(root, cx));
        let changes = cx.new(|cx| ChangesView::new(root, cx));
        let files_events = {
            let changes = changes.clone();
            cx.subscribe_in(&files, window, move |this, _, event: &FilesEvent, window, cx| match event {
                FilesEvent::Open(path) => this.open_file(path, window, cx),
                FilesEvent::GitChanged => {
                    changes.update(cx, ChangesView::reload);
                    this.status_bar.update(cx, StatusBar::refresh);
                }
                FilesEvent::Changed(paths) => {
                    this.editor.update(cx, |editor, cx| editor.files_changed(paths, window, cx));
                }
            })
        };
        let changes_events = Self::follow_changes(&changes, cx);
        // Whether there's a schema to draw is a walk of the folder: off the main thread.
        let folder = root.to_owned();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn({
                    let folder = folder.clone();
                    async move { !saturn_core::schema::find_sources(&folder).is_empty() }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(views) = this.roots.get_mut(&folder) {
                    views.has_schema = found;
                    cx.notify();
                }
            });
        })
        .detach();
        RootViews {
            files,
            changes,
            branch: git::branch(root),
            has_schema: false,
            _events: [files_events, changes_events],
        }
    }

    /// The views of the folder the sidebar shows.
    fn current_root(&self) -> &RootViews {
        self.roots.get(&self.root).or_else(|| self.roots.get(&self.project)).expect("the project's views exist")
    }

    /// Points the sidebar at the focused pane's worktree when following, else at the project.
    fn sync_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane_folder = self
            .follow_pane
            .then(|| self.layout.focused_tab().and_then(|tab| self.terminals.get(&tab)))
            .flatten()
            .map(|view| view.read(cx).cwd.clone());
        let target =
            pane_folder.map_or_else(|| self.project.clone(), |folder| git::toplevel(&folder).unwrap_or(folder));
        if let Some(owner) = self.project_of(&target) {
            self.project = owner;
        }
        if target == self.root {
            return;
        }
        if !self.roots.contains_key(&target) {
            let views = Self::root_views(&target, window, cx);
            self.roots.insert(target.clone(), views);
        }
        self.root = target;
        let root = self.root.clone();
        self.status_bar.update(cx, |bar, cx| bar.set_root(&root, cx));
        cx.notify();
    }

    /// Reviewing a change opens it in the editor panel; new changes refresh an open review.
    fn follow_changes(changes: &Entity<ChangesView>, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe(changes, |this, _, event: &ChangesEvent, cx| match event {
            ChangesEvent::Open { files, scope, path } => {
                this.editor.update(cx, |editor, cx| editor.review(files.clone(), *scope, path, cx));
                this.editor_open = true;
                cx.notify();
            }
            ChangesEvent::Updated { files, scope } => {
                this.editor.update(cx, |editor, cx| editor.refresh_review(files.clone(), *scope, cx));
            }
            ChangesEvent::History { repo } => {
                this.editor.update(cx, |editor, cx| editor.show_history(repo, cx));
                this.editor_open = true;
                cx.notify();
            }
            ChangesEvent::Impact { repo } => {
                this.show_impact(repo, cx);
            }
        })
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let command = self.settings.project(&self.project).ticket_command;
        settings_view::open(cx.weak_entity(), command, window, cx);
    }

    /// The folder the sidebar shows: the selected repo, or one of its worktrees.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The selected repo.
    pub fn project_path(&self) -> &Path {
        &self.project
    }

    /// The repo of this window that `folder` belongs to: the repo itself, a folder inside it, or
    /// one of its worktrees.
    fn project_of(&self, folder: &Path) -> Option<PathBuf> {
        if let Some(project) = self.projects.iter().find(|project| folder.starts_with(project)) {
            return Some(project.clone());
        }
        let main = git::main_worktree(folder)?;
        self.projects.iter().find(|project| **project == main).cloned()
    }

    /// Shows `project` in the sidebar and makes it where new tabs open. Choosing a repo by hand
    /// stops the sidebar following panes, so the choice sticks.
    pub fn select_project(&mut self, project: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_pane = false;
        self.project = project;
        if !self.roots.contains_key(&self.project) {
            let views = Self::root_views(&self.project, window, cx);
            self.roots.insert(self.project.clone(), views);
        }
        self.root = self.project.clone();
        let root = self.root.clone();
        self.status_bar.update(cx, |bar, cx| bar.set_root(&root, cx));
        cx.notify();
    }

    /// Adds a repo to the window (or selects it, if it's here already).
    pub fn add_project(&mut self, project: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        settings::remember_project(&project);
        if !self.projects.contains(&project) {
            self.projects.push(project.clone());
        }
        self.select_project(project, window, cx);
        if self.layout.is_empty() {
            self.open_terminal(Placement::Tab, window, cx);
        }
        self.save_session(cx);
    }

    /// Takes a repo out of the window's list. Its panes stay where they are.
    fn remove_project(&mut self, project: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if self.projects.len() < 2 {
            return;
        }
        self.projects.retain(|p| p != project);
        if self.project == project {
            let first = self.projects[0].clone();
            self.select_project(first, window, cx);
        }
        self.save_session(cx);
        cx.notify();
    }

    pub fn clickup(&self) -> &ClickUpState {
        &self.clickup
    }

    /// Checks `token` with ClickUp and, if it works, keeps it in the Keychain.
    pub fn connect_clickup(&mut self, token: String, cx: &mut Context<Self>) {
        self.clickup.problem = None;
        self.clickup.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let checked = cx
                .background_spawn({
                    let token = token.clone();
                    async move { clickup::check(&token).and_then(|account| clickup::save_token(&token).map(|()| account)) }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.clickup.loading = false;
                match checked {
                    Ok(account) => {
                        this.clickup.token = Some(token);
                        this.clickup.account = Some(account);
                        this.refresh_tickets(cx);
                    }
                    Err(problem) => this.clickup.problem = Some(problem),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn disconnect_clickup(&mut self, cx: &mut Context<Self>) {
        clickup::forget_token();
        self.clickup = ClickUpState::default();
        cx.notify();
    }

    /// Reads the open tickets again, off the main thread.
    pub fn refresh_tickets(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.clickup.token.clone() else { return };
        self.clickup.loading = self.clickup.tickets.is_empty();
        cx.spawn(async move |this, cx| {
            let tickets = cx.background_spawn(async move { clickup::my_tickets(&token) }).await;
            let _ = this.update(cx, |this, cx| {
                this.clickup.loading = false;
                match tickets {
                    Ok(tickets) => {
                        this.clickup.tickets = tickets;
                        this.clickup.problem = None;
                    }
                    Err(problem) => this.clickup.problem = Some(problem),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Starts an agent on `ticket` in a new pane: in its own worktree when the project asks
    /// Saturn to make one, running the project's ticket command.
    pub fn start_ticket(&mut self, ticket: &Ticket, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.settings.project(&self.project);
        let cwd = if project.create_worktree {
            match git::ticket_worktree(&self.project, ticket.key()) {
                Ok(dir) => dir,
                Err(problem) => {
                    self.clickup.problem = Some(format!("No worktree for {}: {problem}", ticket.key()));
                    self.project.clone()
                }
            }
        } else {
            self.project.clone()
        };
        let Some(tab) = self.spawn_terminal(&cwd, window, cx) else { return };
        if self.layout.is_empty() {
            self.layout.add_tab(tab, None);
        } else {
            self.layout.split_right(tab, None);
        }
        let command = format!("{}\r", project.command_for(ticket.key(), &ticket.title));
        if let Some(view) = self.terminals.get(&tab).cloned() {
            // Once the shell has drawn its prompt.
            cx.spawn(async move |_, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(600)).await;
                let () = view.update(cx, |view, _| view.write(command.into_bytes()));
            })
            .detach();
        }
        self.after_layout_change(window, cx);
    }

    /// Reads the branch each tab is on and the pull request of each branch, off the main thread.
    pub fn refresh_checkouts(&mut self, cx: &mut Context<Self>) {
        if self.checkout_refresh != Refresh::Idle {
            self.checkout_refresh = Refresh::RunningStale;
            return;
        }
        self.checkout_refresh = Refresh::Running;
        let dirs: Vec<(TabId, PathBuf)> =
            self.terminals.iter().map(|(tab, view)| (*tab, view.read(cx).cwd.clone())).collect();
        cx.spawn(async move |this, cx| {
            let (checkouts, pull_requests) = cx
                .background_spawn(async move {
                    let checkouts: HashMap<TabId, Checkout> = dirs
                        .into_iter()
                        .filter_map(|(tab, dir)| {
                            let root = git::toplevel(&dir)?;
                            let branch = git::branch(&root)?;
                            Some((tab, Checkout { root, branch }))
                        })
                        .collect();
                    let unique: HashSet<&Checkout> = checkouts.values().collect();
                    let pull_requests: HashMap<Checkout, PullRequest> = unique
                        .into_iter()
                        .filter_map(|checkout| {
                            github::pull_request(&checkout.root, &checkout.branch).map(|pr| (checkout.clone(), pr))
                        })
                        .collect();
                    (checkouts, pull_requests)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let stale = this.checkout_refresh == Refresh::RunningStale;
                this.checkout_refresh = Refresh::Idle;
                this.checkouts = checkouts;
                this.pull_requests = pull_requests;
                cx.notify();
                if stale {
                    this.refresh_checkouts(cx);
                }
            });
        })
        .detach();
    }

    /// The pull request for the branch a tab is on.
    fn pull_request_of(&self, tab: TabId) -> Option<&PullRequest> {
        self.pull_requests.get(self.checkouts.get(&tab)?)
    }

    /// The ticket a tab works on: a ticket whose key is the tab's name, its Claude session's name
    /// or its branch.
    pub fn ticket_of(&self, tab: TabId, cx: &gpui_kit::App) -> Option<&Ticket> {
        if self.clickup.tickets.is_empty() {
            return None;
        }
        let view = self.terminals.get(&tab)?.read(cx);
        let mut names: Vec<String> = Vec::new();
        names.extend(self.names.get(&tab).cloned());
        names.extend(self.session_names.get(&tab).cloned());
        names.extend(view.title.clone());
        names.extend(self.checkouts.get(&tab).map(|checkout| checkout.branch.clone()));
        self.clickup.tickets.iter().find(|ticket| {
            let key = ticket.key().to_lowercase();
            names.iter().any(|name| {
                let name = name.to_lowercase();
                name == key || name.starts_with(&format!("{key}-")) || name.starts_with(&format!("{key} "))
            })
        })
    }

    /// Saves the file in front, formatting it if the setting is on.
    pub fn save_file(&mut self, cx: &mut Context<Self>) {
        let (root, format) = (self.root.clone(), self.settings.format_on_save);
        self.editor.update(cx, |editor, cx| editor.save(&root, format, cx));
    }

    /// Shows the Changes tab and reviews its first file once the changes are read (spikes).
    /// Opens the database diagram for the folder the sidebar shows.
    pub fn show_schema(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        self.editor.update(cx, |editor, cx| {
            editor.show_schema(&root, cx);
            if std::env::var_os("SATURN_SCHEMA").is_some() {
                editor.maximize(cx);
            }
        });
        self.editor_open = true;
        cx.notify();
    }

    /// Makes the editor panel take the grid's room (spikes).
    pub fn maximize_editor(&mut self, cx: &mut Context<Self>) {
        self.editor.update(cx, EditorView::maximize);
    }

    /// Lists the language servers' errors and warnings.
    pub fn show_problems(&mut self, cx: &mut Context<Self>) {
        self.editor.update(cx, EditorView::show_problems);
    }

    /// Asks the open file's language server about `line`:`character` (spikes).
    pub fn probe_language(&mut self, line: u32, character: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| editor.probe_language(line, character, window, cx));
    }

    /// Opens the Impact tab for `repo`.
    pub fn show_impact(&mut self, repo: &Path, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| {
            editor.show_impact(repo, None, cx);
            if std::env::var_os("SATURN_IMPACT").is_some() {
                editor.maximize(cx);
            }
        });
        self.editor_open = true;
        cx.notify();
    }

    /// Opens the History tab for the folder the sidebar shows.
    pub fn show_history(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        self.editor.update(cx, |editor, cx| {
            editor.show_history(&root, cx);
            if std::env::var_os("SATURN_HISTORY").is_some() {
                editor.maximize(cx);
            }
        });
        self.editor_open = true;
        cx.notify();
    }

    /// Gives the panes made-up agents, so screenshots show the Agents tab without running Claude.
    pub fn fake_agents(&mut self, cx: &mut Context<Self>) {
        let samples = [
            (AgentState::Working, Some(40), "Running the tests", 412_000),
            (AgentState::NeedsYou, None, "Which branch should I deploy?", 168_000),
            (AgentState::YourTurn, None, "Done: the filter is on every list page", 96_000),
            (AgentState::Working, Some(72), "Writing the migration", 251_000),
        ];
        let tabs: Vec<TabId> = self.layout.tabs().collect();
        for (tab, (state, progress, message, context)) in tabs.into_iter().zip(samples) {
            if let Some(view) = self.terminals.get(&tab) {
                view.update(cx, |view, _| {
                    view.state = state;
                    view.progress = progress;
                    view.message = Some(message.to_owned());
                });
            }
            let steps = (state == AgentState::Working).then(|| agent_activity::Steps {
                done: 1,
                total: 3,
                current: Some(message.to_owned()),
            });
            self.activity.insert(tab, Activity { steps, context: Some(context) });
            // Not looked at yet, so the panes ring as they would.
            if matches!(state, AgentState::NeedsYou | AgentState::YourTurn) {
                self.unseen.insert(tab);
            }
        }
        cx.notify();
    }

    pub fn review_first_change(&mut self, cx: &mut Context<Self>) {
        self.sidebar_tab = SidebarTab::Changes;
        let changes = self.current_root().changes.clone();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
            changes.update(cx, |changes, cx| changes.open(0, cx));
        })
        .detach();
        cx.notify();
    }

    /// Files / Changes, at the top of the sidebar.
    fn render_sidebar_switch(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab = |label: &'static str, target: SidebarTab, cx: &mut Context<Self>| {
            chrome::track_item(label, self.sidebar_tab == target)
                .flex_1()
                .px(len(space::SPACE_4))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.sidebar_tab = target;
                    cx.notify();
                }))
        };
        chrome::track()
            .child(tab("Files", SidebarTab::Files, cx))
            .child(tab("Changes", SidebarTab::Changes, cx))
            .into_any_element()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn change_settings(&mut self, change: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        let (appearance, menu_bar) = (self.settings.appearance, self.settings.menu_bar);
        change(&mut self.settings);
        settings::save(&self.settings);
        if self.settings.appearance != appearance {
            theme::apply(self.settings.appearance, cx);
        }
        if self.settings.menu_bar != menu_bar {
            menu_bar::set_shown(self.settings.menu_bar);
        }
        cx.notify();
    }

    /// A Claude Code hook from a session in one of the panes.
    fn handle_hook(&mut self, call: HookCall, cx: &mut Context<Self>) {
        let Some(tab) = call.pane.parse().ok().map(TabId) else { return call.done() };
        let Some(view) = self.terminals.get(&tab).cloned() else { return call.done() };
        let session_before = self.claude_sessions.get(&tab).cloned();
        match (call.event(), call.field("session_id")) {
            ("SessionEnd", _) => {
                self.claude_sessions.remove(&tab);
            }
            (_, Some(session)) => {
                self.claude_sessions.insert(tab, session.to_owned());
            }
            _ => {}
        }
        if self.claude_sessions.get(&tab) != session_before.as_ref() {
            self.save_session(cx);
        }
        self.track_activity(tab, &call, cx);
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
        // In the focused pane's folder when it's in the repo the sidebar shows, else at that repo's top.
        let pane_folder = self
            .layout
            .focused_tab()
            .and_then(|focused| self.terminals.get(&focused))
            .map(|view| view.read(cx).cwd.clone());
        let cwd = pane_folder.filter(|folder| folder.starts_with(&self.root)).unwrap_or_else(|| self.root.clone());
        let tab = self.spawn_terminal(&cwd, window, cx)?;
        match placement {
            Placement::Tab => self.layout.add_tab(tab, None),
            Placement::Right => self.layout.split_right(tab, None),
            Placement::Down => self.layout.split_down(tab, None),
        }
        self.after_layout_change(window, cx);
        Some(tab)
    }

    /// Starts a shell in `cwd` in a new tab that is not in the grid yet.
    fn spawn_terminal(&mut self, cwd: &Path, window: &mut Window, cx: &mut Context<Self>) -> Option<TabId> {
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
        let (terminal, output) = match TerminalView::spawn(cwd, &env) {
            Ok(started) => started,
            Err(error) => {
                eprintln!("saturn: could not start a shell in {}: {error:#}", cwd.display());
                return None;
            }
        };
        let view = cx.new(|cx| TerminalView::new(terminal, output, cwd, cx));
        cx.subscribe_in(&view, window, move |this, _view, event, window, cx| {
            this.terminal_event(tab, event, window, cx);
        })
        .detach();
        self.terminals.insert(tab, view);
        self.refresh_checkouts(cx);
        Some(tab)
    }

    /// Opens the grid as it was: each tab in its folder, with its name, and a Claude session's
    /// resume command typed (not run) where one was running. False when nothing was saved.
    pub fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(saved) = session::load(&self.project) else { return false };
        for project in &saved.projects {
            if project.is_dir() && !self.projects.contains(project) {
                self.projects.push(project.clone());
                let views = Self::root_views(project, window, cx);
                self.roots.insert(project.clone(), views);
            }
        }
        let mut tabs = Vec::new();
        for saved_tab in &saved.tabs {
            let cwd = if saved_tab.cwd.is_dir() { saved_tab.cwd.clone() } else { self.project.clone() };
            let Some(tab) = self.spawn_terminal(&cwd, window, cx) else { continue };
            if let Some(name) = &saved_tab.name {
                self.names.insert(tab, name.clone());
            }
            if let Some(command) = saved_tab.claude_session.as_deref().and_then(session::resume_command)
                && let Some(view) = self.terminals.get(&tab).cloned()
            {
                // Once the shell has drawn its prompt; typed text waits there for Enter.
                cx.spawn(async move |_, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(800)).await;
                    let () = view.update(cx, |view, _| view.write(command.into_bytes()));
                })
                .detach();
            }
            tabs.push(tab);
        }
        if tabs.is_empty() {
            return false;
        }
        self.layout = Layout::from_shape(&saved.shape, &tabs);
        self.after_layout_change(window, cx);
        true
    }

    /// Remembers the grid for the next start.
    fn save_session(&self, cx: &gpui_kit::App) {
        let tabs: Vec<TabId> = self.layout.tabs().collect();
        let home = &self.projects[0];
        let grid = (!tabs.is_empty()).then(|| SavedGrid {
            projects: self.projects[1..].to_vec(),
            shape: self.layout.shape(),
            tabs: tabs
                .iter()
                .map(|tab| SavedTab {
                    cwd: self.terminals.get(tab).map_or_else(|| self.project.clone(), |view| view.read(cx).cwd.clone()),
                    name: self.names.get(tab).cloned(),
                    claude_session: self.claude_sessions.get(tab).cloned(),
                })
                .collect(),
        });
        session::save(home, grid);
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
                if asks && !window.is_window_active() && self.settings.notifications {
                    self.notify_about(tab, cx);
                }
                if asks && !self.is_looking_at(tab, window) {
                    self.unseen.insert(tab);
                } else if !asks {
                    self.unseen.remove(&tab);
                }
                self.report_attention(window, cx);
                cx.notify();
            }
            TerminalViewEvent::Bell => {
                if !self.is_looking_at(tab, window) {
                    self.unseen.insert(tab);
                }
                self.report_attention(window, cx);
                cx.notify();
            }
            TerminalViewEvent::TitleChanged => cx.notify(),
            TerminalViewEvent::CwdChanged => {
                if self.layout.focused_tab() == Some(tab) {
                    self.sync_root(window, cx);
                }
                self.refresh_checkouts(cx);
                self.save_session(cx);
            }
        }
    }

    /// A macOS notification about a tab that needs you or finished, while Saturn is in the background.
    fn notify_about(&self, tab: TabId, cx: &Context<Self>) {
        let Some(view) = self.terminals.get(&tab) else { return };
        let (state, message) = {
            let view = view.read(cx);
            (view.state, view.message.clone())
        };
        let name = self.tab_name(tab, cx);
        let (title, fallback, sound) = match state {
            AgentState::NeedsYou => (format!("{name} needs you"), "Waiting for your answer", true),
            AgentState::Error => (format!("{name} stopped"), "The turn failed", true),
            _ => (format!("{name} is done"), "Your turn", false),
        };
        let body = message.filter(|message| !message.is_empty()).unwrap_or_else(|| fallback.to_owned());
        notify::post(&format!("tab-{}", tab.0), &title, &body, sound);
    }

    /// Tells the Dock and the menu bar how many agents wait for you. Called when states change,
    /// not only on redraw: macOS doesn't redraw a hidden window, which is when these matter.
    fn report_attention(&mut self, window: &Window, cx: &gpui_kit::App) {
        if self.unseen.len() != self.badge {
            self.badge = self.unseen.len();
            dock::set_badge(self.badge);
        }
        let waiting = self.terminals.values().filter(|view| view.read(cx).state.asks_for_you()).count();
        if waiting != self.waiting {
            self.waiting = waiting;
            menu_bar::set_waiting(window.window_handle().window_id().as_u64(), waiting);
        }
    }

    /// Keeps an agent's steps (from its task list) and context (from its transcript) current.
    fn track_activity(&mut self, tab: TabId, call: &HookCall, cx: &mut Context<Self>) {
        if call.event() == "SessionEnd" {
            self.activity.remove(&tab);
            return;
        }
        if call.event() == "PostToolUse"
            && let (Some(name), Some(input)) = (call.field("tool_name"), call.input.get("tool_input"))
            && let Some(steps) = agent_activity::steps(name, input)
        {
            self.activity.entry(tab).or_default().steps = Some(steps);
        }
        if matches!(call.event(), "PostToolUse" | "Stop")
            && let Some(transcript) = call.field("transcript_path").map(PathBuf::from)
        {
            cx.spawn(async move |this, cx| {
                let context = cx
                    .background_spawn(async move {
                        read_tail(&transcript).and_then(|tail| agent_activity::context_tokens(&tail))
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(context) = context {
                        this.activity.entry(tab).or_default().context = Some(context);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    /// The focused pane's agent as the gradient card, when that pane runs Claude.
    fn render_focused_agent(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let tab = self.layout.focused_tab()?;
        let view = self.terminals.get(&tab)?.read(cx);
        if view.state == AgentState::Shell && !self.claude_sessions.contains_key(&tab) {
            return None;
        }
        let activity = self.activity.get(&tab);
        let agent = agents_view::Agent {
            name: self.tab_name(tab, cx),
            state: view.state,
            message: view.message.clone(),
            progress: view.progress,
            steps: activity.and_then(|activity| activity.steps.clone()),
            context: activity.and_then(|activity| activity.context),
        };
        Some(agents_view::card("focused-agent", &agent).into_any_element())
    }

    fn forget(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        self.terminals.remove(&tab);
        self.unseen.remove(&tab);
        self.names.remove(&tab);
        self.checkouts.remove(&tab);
        self.activity.remove(&tab);
        self.layout.remove_tab(tab);
        self.report_attention(window, cx);
        self.after_layout_change(window, cx);
    }

    /// Focuses the terminal now in front and marks it seen.
    fn after_layout_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_root(window, cx);
        self.save_session(cx);
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

    /// Where a tab being dragged over `cell_id` would land. Every pane hears every move of a drag;
    /// only the one under the pointer answers.
    fn track_drop(&mut self, cell_id: CellId, event: &DragMoveEvent<DraggedTab>, cx: &mut Context<Self>) {
        let target = event
            .bounds
            .contains(&event.event.position)
            .then(|| (cell_id, drop_side(event.bounds, event.event.position)));
        let mine = self.drop_target.is_some_and(|(cell, _)| cell == cell_id);
        if target.is_some() && self.drop_target != target {
            self.drop_target = target;
            cx.notify();
        } else if target.is_none() && mine {
            self.drop_target = None;
            cx.notify();
        }
    }

    /// A tab dropped on a pane: split the pane on the side it landed, or move into it.
    fn drop_on_pane(&mut self, tab: TabId, cell_id: CellId, window: &mut Window, cx: &mut Context<Self>) {
        let side = self.drop_target.filter(|(cell, _)| *cell == cell_id).and_then(|(_, side)| side);
        self.drop_target = None;
        match side {
            Some(side) => self.update_layout(window, cx, |layout| layout.split_with(tab, cell_id, side)),
            None => self.drop_tab(tab, cell_id, None, window, cx),
        }
    }

    /// Asks for a folder and adds it to the window's repos.
    fn open_project(window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add Repo".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            let Some(project) = paths.into_iter().next() else { return };
            let _ = this.update_in(cx, |this, window, cx| this.add_project(project, window, cx));
        })
        .detach();
    }

    fn render_tab(&self, cell: &Cell, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let tab = cell.tabs[index];
        let cell_id = cell.id;
        let selected = tab == cell.active;
        let state = self.tab_state(tab, cx);
        let name = self.tab_name(tab, cx);
        // A tab hidden behind another that needs you turns orange, so it can't be missed.
        let calling = !selected && state == AgentState::NeedsYou && self.unseen.contains(&tab);
        let on_orange = if saturn_design::appearance() == saturn_design::Appearance::Dark {
            theme::color(color::INK_ON_SIGNAL)
        } else {
            gpui_kit::white()
        };
        let foreground = match (calling, selected) {
            (true, _) => on_orange,
            (false, true) => theme::color(color::INK),
            (false, false) => theme::color(color::INK_MUTED),
        };
        let renaming = self.renaming.as_ref().filter(|(renaming, _)| *renaming == tab).map(|(_, input)| input.clone());

        let close = div()
            .id(("close", tab.0))
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.))
            .rounded(len(radius::RADIUS_XS))
            .when(!selected, |close| close.invisible().group_hover("tab", gpui_kit::Styled::visible))
            .hover(|s| s.bg(theme::fill_control()))
            .child(chrome::icon("x", 11., foreground))
            .tooltip(chrome::tip_for("Close this terminal", CloseTab))
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
            .rounded(len(radius::RADIUS_SM))
            // Selected the way every tab is (chrome::raised); a hidden tab that needs you, orange.
            .map(|tab| chrome::raised(tab, selected && !calling))
            .when(calling, |tab| tab.bg(theme::color(color::STATUS_NEEDS_YOU)))
            .when(!selected && !calling, |tab| tab.hover(|s| s.bg(theme::fill_hover())))
            .text_color(foreground)
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
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
            .drag_over::<DraggedTab>(|style, _, _, _| style.border_l_2().border_color(theme::color(color::BLUE)))
            .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                this.drop_tab(dragged.tab, cell_id, Some(index), window, cx);
            }))
            .into_any_element()
    }

    fn render_cell(&self, cell: &Cell, cx: &mut Context<Self>) -> AnyElement {
        let focused = self.layout.focused == Some(cell.id);
        let unseen_states: Vec<AgentState> =
            cell.tabs.iter().filter(|tab| self.unseen.contains(tab)).map(|tab| self.tab_state(*tab, cx)).collect();
        // Orange for anything waiting on you, a stopped turn included; green for a finished one.
        let needs_you = unseen_states.iter().any(|state| matches!(state, AgentState::NeedsYou | AgentState::Error));
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
            let button = chrome::icon_button((id, cell_id.key()), icon, true)
                .on_click(cx.listener(move |this, _, window, cx| this.open_in_cell(cell_id, placement, window, cx)));
            match placement {
                Placement::Right => {
                    button.tooltip(chrome::tip_for("Split right: a new terminal beside this one", SplitRight))
                }
                Placement::Down => {
                    button.tooltip(chrome::tip_for("Split down: a new terminal below this one", SplitDown))
                }
                Placement::Tab => button.tooltip(chrome::tip_for("New terminal, as a tab here", NewTab)),
            }
        };

        // A panel like every other: the tabs in its strip, the terminal in its well.
        let mut pane = chrome::panel()
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
                    .flex_none()
                    .items_center()
                    .gap(len(space::SPACE_1))
                    .h(len(TAB_HEIGHT))
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
                    .children(self.ticket_of(cell.active, cx).map(|ticket| ticket_chip(cell_id, ticket)))
                    .children(self.pull_request_of(cell.active).map(|pr| pull_request_chip(cell_id, pr)))
                    .child(header_button("split-right", "columns-2", Placement::Right, cx))
                    .child(header_button("split-down", "rows-2", Placement::Down, cx))
                    .child(header_button("new-tab", "plus", Placement::Tab, cx)),
            )
            .child(
                chrome::well()
                    .id(("body", cell_id.key()))
                    .pt(len(space::SPACE_3))
                    .pl(len(space::SPACE_4))
                    .pb(len(space::SPACE_1))
                    .on_drag_move(cx.listener(move |this, event: &DragMoveEvent<DraggedTab>, _, cx| {
                        this.track_drop(cell_id, event, cx);
                    }))
                    .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                        this.drop_on_pane(dragged.tab, cell_id, window, cx);
                    }))
                    .children(terminal)
                    .children(
                        self.drop_target
                            .filter(|(cell, _)| *cell == cell_id && cx.has_active_drag())
                            .map(|(_, side)| drop_preview(side)),
                    ),
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
                    |pane, t| pane.shadow(theme::layers(&breathing_glow(t))),
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
            .when(vertical, |d| d.w(len(space::SPACE_2)).h_full().cursor(CursorStyle::ResizeLeftRight))
            .when(!vertical, |d| d.h(len(space::SPACE_2)).w_full().cursor(CursorStyle::ResizeUpDown))
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
            (count(&[AgentState::NeedsYou, AgentState::Error]), AgentState::NeedsYou, "need you"),
            (count(&[AgentState::YourTurn]), AgentState::YourTurn, "your turn"),
            (count(&[AgentState::Working]), AgentState::Working, "working"),
        ];
        // The title bar is drawn by Saturn, so the empty parts of it pass a double-click on to
        // macOS, which zooms or minimizes as System Settings says.
        let empty = |id: &'static str| {
            div().id(id).h_full().on_click(|event: &ClickEvent, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
        };
        // Controls live in tracks, as everywhere: the panel toggles on the left (raised while the
        // panel is open), what the agents are doing on the right, then the one primary action.
        let toggle = |id: &'static str, icon: &str, on: bool| {
            chrome::track_item(id, on).child(chrome::icon(
                icon,
                16.,
                theme::color(if on { color::INK } else { color::INK_MUTED }),
            ))
        };
        let live: Vec<(usize, AgentState, &str)> = summary.into_iter().filter(|(n, _, _)| *n > 0).collect();
        let activity = (!live.is_empty()).then(|| {
            let mut track = chrome::track();
            for (index, (n, state, label)) in live.into_iter().enumerate() {
                if index > 0 {
                    track = track.child(chrome::track_separator());
                }
                track = track.child(
                    chrome::track_item(label, false)
                        .px(len(space::SPACE_3))
                        .when(state == AgentState::Working, |item| {
                            item.bg(theme::color(color::BLUE_VIVID).opacity(0.14)).text_color(theme::color(color::BLUE))
                        })
                        .when(state != AgentState::Working, |item| item.text_color(state_color(state)))
                        .child(status_dot(state))
                        .child(format!("{n} {label}"))
                        .tooltip(chrome::tip_for("Go to the next agent that needs you", NextAttention))
                        .on_click(cx.listener(|this, _, window, cx| this.next_attention(window, cx))),
                );
            }
            track
        });
        div()
            .flex()
            .items_center()
            .gap(len(space::SPACE_2))
            .h(len(TITLEBAR_HEIGHT))
            .pr(len(space::SPACE_1))
            .child(empty("titlebar-lights").flex_none().w(len(TRAFFIC_LIGHTS_WIDTH)))
            .child(
                chrome::track()
                    .child(
                        toggle("toggle-sidebar", "panel-left", self.sidebar_open)
                            .tooltip(chrome::tip_for("Show or hide the sidebar", ToggleSidebar))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sidebar_open = !this.sidebar_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        toggle("toggle-editor", "panel-top", self.editor_open)
                            .tooltip(chrome::tip_for("Show or hide the editor", ToggleEditor))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editor_open = !this.editor_open;
                                cx.notify();
                            })),
                    )
                    .child(chrome::track_separator())
                    .child(
                        toggle("open-settings", "settings", false)
                            .tooltip(chrome::tip_for("Settings", OpenSettings))
                            .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
                    ),
            )
            .child(self.render_repo_tabs(cx))
            .child(empty("titlebar-middle").flex_1())
            .children(activity)
            .child(
                // New terminals come from each pane's ＋ (and ⌘T); the title bar adds repos. One
                // primary button per screen: New agent when ClickUp is connected, else this, unless
                // the empty grid shows its own.
                chrome::button(
                    "add-repo",
                    "Add repo",
                    Some("⌘O"),
                    !self.clickup.connected() && !self.layout.is_empty(),
                )
                .h(len(chrome::TRACK_HEIGHT))
                .rounded(len(radius::RADIUS_MD))
                .tooltip(chrome::tip("Open another repository in this window"))
                .on_click(cx.listener(|_, _, window, cx| Self::open_project(window, cx))),
            )
            .children(self.clickup.connected().then(|| {
                chrome::button("new-agent", "New agent", Some("⌘⇧T"), true)
                    .h(len(chrome::TRACK_HEIGHT))
                    .rounded(len(radius::RADIUS_MD))
                    .tooltip(chrome::tip("Start an agent on a ClickUp ticket, in its own worktree"))
                    .on_click(cx.listener(|_, _, window, cx| ticket_picker::open(cx.weak_entity(), window, cx)))
            }))
    }

    /// The sidebar's header: the folder it shows (the selected repo, or a ticket's worktree when
    /// following a pane in one), and the follow switch.
    fn render_sidebar_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let short = match home.as_deref().and_then(|home| self.root.strip_prefix(home).ok()) {
            Some(rest) => format!("~/{}", rest.display()),
            None => self.root.display().to_string(),
        };
        let name = self.root.file_name().map_or_else(|| "/".to_owned(), |name| name.to_string_lossy().into_owned());
        let branch = self.roots.get(&self.root).and_then(|views| views.branch.clone());
        let detail = branch.map_or_else(|| short.clone(), |branch| format!("{branch} · {short}"));
        let follow = chrome::icon_button("follow-pane", if self.follow_pane { "locate-fixed" } else { "locate" }, true)
            .when(self.follow_pane, |button| button.bg(theme::fill_pressed()))
            .tooltip(chrome::tip("Follow the focused terminal: the sidebar shows the folder it works in"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.follow_pane = !this.follow_pane;
                this.sync_root(window, cx);
                cx.notify();
            }));
        // The database diagram, when this folder describes a database.
        let schema = self.roots.get(&self.root).is_some_and(|views| views.has_schema).then(|| {
            chrome::icon_button("show-schema", "database", true)
                .tooltip(chrome::tip("Database diagram: the tables and how they connect"))
                .on_click(cx.listener(|this, _, _, cx| this.show_schema(cx)))
        });
        chrome::sidebar_header(&name, &detail, div().flex().items_center().children(schema).child(follow))
            .into_any_element()
    }

    /// The window's repos, in the title bar: the selected one raised, ✕ on hover to take one out
    /// (its panes stay). Adding one is the title bar's button on the right.
    fn render_repo_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let removable = self.projects.len() > 1;
        let mut track = chrome::track();
        for (index, project) in self.projects.iter().enumerate() {
            let selected = *project == self.project;
            let name = project.file_name().map_or_else(|| "/".to_owned(), |name| name.to_string_lossy().into_owned());
            let picked = project.clone();
            let removed = project.clone();
            // Each tab its own hover group, so only the one under the mouse shows its ✕.
            let group = gpui_kit::SharedString::from(format!("repo-{index}"));
            track = track.child(
                chrome::track_item(("repo", index), selected)
                    .relative()
                    .group(group.clone())
                    .px(len(space::SPACE_3))
                    .text_color(theme::color(if selected { color::INK } else { color::INK_MUTED }))
                    .child(name)
                    // On hover only, floating over the tab's end, so a hidden ✕ takes no room.
                    .when(removable, |item| {
                        item.child(
                            div()
                                .id(("remove-repo", index))
                                .absolute()
                                .top(px(5.))
                                .right(px(5.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(18.))
                                .rounded(len(radius::RADIUS_XS))
                                .bg(theme::color(color::SURFACE))
                                .shadow(vec![gpui_kit::BoxShadow {
                                    color: theme::overlay(0.12),
                                    offset: gpui_kit::point(px(0.), px(0.)),
                                    blur_radius: px(0.),
                                    spread_radius: px(1.),
                                    inset: false,
                                }])
                                .invisible()
                                .group_hover(group, gpui_kit::Styled::visible)
                                .child(chrome::icon("x", 11., theme::color(color::INK_MUTED)))
                                .tooltip(chrome::tip("Close this repo here (its terminals keep running)"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.remove_project(&removed, window, cx);
                                })),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| this.select_project(picked.clone(), window, cx))),
            );
        }
        track.into_any_element()
    }

    /// Sidebar | (editor / grid), with draggable dividers. The panels stay in the same place in
    /// the tree when toggled, so terminals never restart.
    fn render_workbench(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor_maximized = self.editor.read(cx).maximized();
        let half_gap = len(space::SPACE_2 / 2.);
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
        let switch = self.render_sidebar_switch(cx);
        let root = self.current_root();
        let (files, changes) = (root.files.clone(), root.changes.clone());
        let header = self.render_sidebar_header(cx);
        let agent = self.render_focused_agent(cx);
        let sidebar = match self.sidebar_tab {
            SidebarTab::Files => chrome::sidebar(header, switch, files, agent),
            SidebarTab::Changes => chrome::sidebar(header, switch, changes, agent),
        };
        div().flex_1().min_h_0().child(
            h_resizable("workbench")
                .with_handle_appearance(chrome::quiet_resize_handle())
                .child(
                    resizable_panel()
                        .size(px(270.))
                        .size_range(px(200.)..px(480.))
                        .visible(self.sidebar_open)
                        .child(div().size_full().pr(half_gap).child(sidebar)),
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
                                        .child(div().size_full().pb(half_gap).child(self.editor.clone())),
                                )
                                .child(
                                    resizable_panel().visible(!(self.editor_open && editor_maximized)).child(
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

impl Workspace {
    /// Copy and paste in the focused terminal.
    fn clipboard_actions(element: gpui_kit::Div, cx: &mut Context<Self>) -> gpui_kit::Div {
        element
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
    }

    /// The palettes, and the views the command palette opens by name.
    fn palette_actions(element: gpui_kit::Div, cx: &mut Context<Self>) -> gpui_kit::Div {
        element
            .on_action(cx.listener(|this, _: &GoToFile, window, cx| {
                palette::go_to_file(cx.weak_entity(), this.root.clone(), window, cx);
            }))
            .on_action(cx.listener(|_, _: &CommandPalette, window, cx| palette::commands(window, cx)))
            .on_action(cx.listener(|this, _: &ShowHistory, _, cx| this.show_history(cx)))
            .on_action(cx.listener(|this, _: &ShowImpact, _, cx| {
                let root = this.root.clone();
                this.show_impact(&root, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowSchema, _, cx| this.show_schema(cx)))
            .on_action(cx.listener(|this, _: &ShowProblems, _, cx| this.show_problems(cx)))
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.report_attention(window, cx);
        // With nothing holding the keyboard (Saturn just came forward, a dialog just closed),
        // shortcuts would reach nothing: the workspace takes it.
        if window.focused(cx).is_none() {
            let focus = self.focus.clone();
            cx.defer_in(window, move |_, window, cx| {
                if window.focused(cx).is_none() {
                    window.focus(&focus, cx);
                }
            });
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .px(len(space::SPACE_2))
            // No fill: the title bar and the gaps are the window's glass (theme::overlay). The
            // status bar's card sets the space below.
            .text_color(theme::color(color::INK))
            .font_family(saturn_design::font::SANS)
            .track_focus(&self.focus)
            .key_context("Workspace")
            .map(|element| Self::palette_actions(element, cx))
            .map(|element| Self::clipboard_actions(element, cx))
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
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)))
            .on_action(cx.listener(|this, _: &SaveFile, _, cx| this.save_file(cx)))
            .on_action(cx.listener(|_, _: &OpenTicketPicker, window, cx| {
                ticket_picker::open(cx.weak_entity(), window, cx);
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
            .child(self.status_bar.clone())
    }
}

/// The last 256 KB of a transcript: enough for its newest replies, without reading a long session whole.
fn read_tail(path: &Path) -> Option<String> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(256 * 1024))).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{Direction, drop_side, move_divider};
    use gpui_kit::{Bounds, point, px, size};

    #[test]
    fn a_dropped_tab_splits_on_the_nearest_edge_or_joins_in_the_middle() {
        let pane = Bounds::new(point(px(100.), px(50.)), size(px(400.), px(200.)));
        let at = |x: f32, y: f32| drop_side(pane, point(px(x), px(y)));
        assert_eq!(at(110., 150.), Some(Direction::Left));
        assert_eq!(at(490., 150.), Some(Direction::Right));
        assert_eq!(at(300., 60.), Some(Direction::Up));
        assert_eq!(at(300., 240.), Some(Direction::Down));
        assert_eq!(at(300., 150.), None);
    }

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
