//! The window's root: the title bar and the grid of terminal panes, each with tabs.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui_kit::component::resizable::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, Focusable as _, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, Window, actions, div, prelude::FluentBuilder as _, px, relative,
};
use saturn_core::agent_state::{AgentState, tab_title};
use saturn_core::layout::{Cell, CellId, Direction, Layout, TabId};
use saturn_design::{color, radius, shadow, space};
use saturn_terminal::view::{TerminalView, TerminalViewEvent};

use crate::chrome::{self, TileKind};
use crate::theme::{self, len};

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
const PANE_HEADER_HEIGHT: f32 = 34.0;
const TAB_HEIGHT: f32 = 26.0;

pub struct Workspace {
    project: PathBuf,
    layout: Layout,
    terminals: HashMap<TabId, Entity<TerminalView>>,
    /// Tabs that changed into a state asking for the user while they looked elsewhere.
    unseen: HashSet<TabId>,
    next_tab: u64,
    focus: FocusHandle,
    sidebar_open: bool,
    editor_open: bool,
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

impl Workspace {
    pub fn new(project: PathBuf, cx: &mut Context<Self>) -> Self {
        Self {
            project,
            layout: Layout::default(),
            terminals: HashMap::new(),
            unseen: HashSet::new(),
            next_tab: 0,
            focus: cx.focus_handle(),
            sidebar_open: true,
            editor_open: true,
        }
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
        let pane_id = tab.0.to_string();
        let env = [
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("TERM_PROGRAM", "Saturn"),
            ("SATURN_PANE_ID", pane_id.as_str()),
        ];
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
        self.layout.remove_tab(tab);
        self.after_layout_change(window, cx);
    }

    /// Focuses the terminal now in front and marks it seen.
    fn after_layout_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(front) = self.layout.focused_tab() {
            self.unseen.remove(&front);
            if let Some(view) = self.terminals.get(&front) {
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

    fn close_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        // The tab goes when its shell reports the exit.
        if let Some(view) = self.terminals.get(&tab) {
            view.update(cx, |view, _| view.kill());
        }
    }

    fn tab_name(&self, tab: TabId, cx: &Context<Self>) -> String {
        self.terminals
            .get(&tab)
            .and_then(|view| view.read(cx).title.as_deref().and_then(tab_title))
            .unwrap_or_else(|| "zsh".to_owned())
    }

    fn tab_state(&self, tab: TabId, cx: &Context<Self>) -> AgentState {
        self.terminals.get(&tab).map_or(AgentState::Shell, |view| view.read(cx).state)
    }

    fn render_tab(&self, cell: &Cell, tab: TabId, cx: &mut Context<Self>) -> AnyElement {
        let selected = tab == cell.active;
        let state = self.tab_state(tab, cx);
        // A tab hidden behind another that needs you turns yellow, so it can't be missed.
        let calling = !selected && state == AgentState::NeedsYou && self.unseen.contains(&tab);
        let (background, foreground) = match (calling, selected) {
            (true, _) => (theme::color(color::YELLOW), theme::color(color::INK_INVERSE)),
            (false, true) => (theme::color(color::CONTROL), theme::color(color::INK)),
            (false, false) => (gpui_kit::transparent_black(), theme::color(color::INK_MUTED)),
        };
        div()
            .id(("tab", tab.0))
            .flex()
            .items_center()
            .gap(len(6.))
            .h(len(TAB_HEIGHT))
            .px(len(space::SPACE_3))
            .rounded(len(radius::RADIUS_MD))
            .bg(background)
            .text_color(foreground)
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(status_dot(state))
            .child(self.tab_name(tab, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.update_layout(window, cx, |layout| layout.activate_tab(tab));
                }),
            )
            .into_any_element()
    }

    fn render_cell(&self, cell: &Cell, cx: &mut Context<Self>) -> AnyElement {
        let focused = self.layout.focused == Some(cell.id);
        let unseen_states: Vec<AgentState> =
            cell.tabs.iter().filter(|tab| self.unseen.contains(tab)).map(|tab| self.tab_state(*tab, cx)).collect();
        let glow = if unseen_states.contains(&AgentState::NeedsYou) {
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

        let mut pane = div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .rounded(len(radius::RADIUS_LG))
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
                    .flex()
                    .items_center()
                    .gap(len(space::SPACE_1))
                    .h(len(PANE_HEADER_HEIGHT))
                    .p(len(space::SPACE_1))
                    .bg(theme::color(color::SURFACE_RAISED))
                    .children(cell.tabs.iter().map(|tab| self.render_tab(cell, *tab, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .pt(len(space::SPACE_3))
                    .pl(len(space::SPACE_4))
                    .pb(len(space::SPACE_1))
                    .children(terminal),
            );
        if let Some(glow) = glow {
            pane = pane.border_color(gpui_kit::transparent_black()).shadow(theme::shadows(glow));
        }
        pane.into_any_element()
    }

    fn render_grid(&self, cx: &mut Context<Self>) -> AnyElement {
        let maximized = self.layout.maximized;
        let mut grid = div().flex().flex_col().flex_1().min_h_0().gap(len(space::SPACE_2));
        for row in &self.layout.rows {
            let shown: Vec<&Cell> = row.cells.iter().filter(|cell| maximized.is_none_or(|id| id == cell.id)).collect();
            if shown.is_empty() {
                continue;
            }
            let mut row_element = div().flex().gap(len(space::SPACE_2)).min_h_0().flex_basis(relative(0.));
            row_element.style().flex_grow = Some(if maximized.is_some() { 1.0 } else { row.size });
            for cell in shown {
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
            chrome::icon_tile(id, icon, if on { TileKind::Plain } else { TileKind::Off }, 26.)
        };
        div()
            .flex()
            .items_center()
            .gap(len(space::SPACE_5))
            .h(len(TITLEBAR_HEIGHT))
            .pl(len(TRAFFIC_LIGHTS_WIDTH))
            .pr(len(space::SPACE_1))
            .text_size(px(12.))
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
            .child(div().flex_1())
            .children(summary.into_iter().filter(|(n, _, _)| *n > 0).map(|(n, state, label)| {
                div()
                    .flex()
                    .items_center()
                    .gap(len(6.))
                    .text_color(if state == AgentState::Working {
                        theme::color(color::INK_MUTED)
                    } else {
                        state_color(state)
                    })
                    .child(status_dot(state))
                    .child(format!("{n} {label}"))
            }))
            .child(
                div()
                    .id("new-terminal")
                    .flex()
                    .items_center()
                    .gap(len(space::SPACE_2))
                    .h(px(30.))
                    .px(len(space::SPACE_4))
                    .rounded(len(radius::RADIUS_MD))
                    .bg(theme::color(color::PURPLE))
                    .hover(|s| s.bg(theme::color(color::PURPLE_DEEP)))
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .child("New terminal")
                    .child(div().text_color(theme::color(color::INK).opacity(0.65)).child("⌘T"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_terminal(Placement::Tab, window, cx);
                    })),
            )
    }

    /// Sidebar | (editor / grid), with draggable dividers. The panels stay in the same place in
    /// the tree when toggled, so terminals never restart.
    fn render_workbench(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let half_gap = len(space::SPACE_2 / 2.);
        let grid = if self.layout.is_empty() {
            chrome::empty_grid(cx.listener(|this, _, window, cx| {
                this.open_terminal(Placement::Tab, window, cx);
            }))
        } else {
            self.render_grid(cx)
        };
        let name = self.project.file_name().map_or_else(|| "/".to_owned(), |name| name.to_string_lossy().into_owned());
        let path = self.project.display().to_string();
        div().flex_1().min_h_0().child(
            h_resizable("workbench")
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
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .px(len(space::SPACE_2))
            .pb(len(space::SPACE_2))
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
            .on_action(cx.listener(|this, _: &CloseTab, _, cx| {
                if let Some(tab) = this.layout.focused_tab() {
                    this.close_tab(tab, cx);
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
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if let Some(view) = this.layout.focused_tab().and_then(|tab| this.terminals.get(&tab)).cloned() {
                    view.update(cx, |view, cx| view.copy_selection(cx));
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                if let Some(view) = this.layout.focused_tab().and_then(|tab| this.terminals.get(&tab)).cloned() {
                    view.update(cx, saturn_terminal::view::TerminalView::paste_from_clipboard);
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
