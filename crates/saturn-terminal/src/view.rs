//! A terminal as a GPUI view: owns the [`Terminal`], turns its output into events for the app,
//! and handles keys, paste and scrolling.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use alacritty_terminal::index::Side;
use alacritty_terminal::term::TermMode;
use gpui_kit::{
    App, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels,
    Point, Render, ScrollDelta, ScrollWheelEvent, Styled as _, Window, div,
};
use saturn_core::agent_state::AgentState;

use crate::element::TerminalElement;
use crate::keys::{self, KeyPress};
use crate::pty::PtyEvent;
use crate::terminal::{GridSize, Terminal, TerminalEvent};

gpui_kit::actions!(terminal, [SendTab, SendBackTab]);

/// Option acts as Meta in every terminal (Settings → Terminal).
pub static OPTION_AS_META: AtomicBool = AtomicBool::new(false);
/// Selecting text copies it when the mouse is let go (Settings → Terminal).
pub static COPY_ON_SELECT: AtomicBool = AtomicBool::new(false);

/// Tab and shift-tab belong to the shell (completion, Claude Code's suggestions). The window binds
/// them to moving focus between controls, and bindings run before a view sees its keys, so the
/// terminal binds them again, closer in: the nearer binding wins.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", SendTab, Some("Terminal")),
        KeyBinding::new("shift-tab", SendBackTab, Some("Terminal")),
    ]);
}

/// What a terminal tells the app.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalViewEvent {
    TitleChanged,
    StateChanged,
    Bell,
    /// The shell moved to another folder (OSC 7), such as an agent's worktree.
    CwdChanged,
    /// The shell ended; the tab should close.
    Exited,
}

/// Where the grid was last drawn: enough to turn a mouse position into a cell.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub origin: Point<Pixels>,
    pub cell_width: Pixels,
    pub line_height: Pixels,
}

/// What a held mouse button is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    None,
    Selecting,
    /// The program asked for mouse events (Claude's full-screen view does); they go to it.
    Reporting,
}

pub struct TerminalView {
    terminal: Terminal,
    focus: FocusHandle,
    /// The terminal's own title as set by the program.
    pub title: Option<String>,
    pub state: AgentState,
    /// Claude's current activity or question (OSC 7501 `msg`).
    pub message: Option<String>,
    /// How far Claude says it is, 0–100 (OSC 7501 `progress`), when it says.
    pub progress: Option<u8>,
    pub cwd: PathBuf,
    /// Claude reports its state itself (OSC 7501); hook-based state is then ignored.
    has_program_status: bool,
    scroll_remainder: f32,
    geometry: Rc<Cell<Option<Geometry>>>,
    drag: Drag,
    /// The cell a drag the program follows was last reported at, so each cell is reported once.
    reported: Option<(usize, usize)>,
}

impl EventEmitter<TerminalViewEvent> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TerminalView {
    /// Starts a shell in `cwd` (with `extra_env` added to its environment) for a new view.
    pub fn spawn(
        cwd: &Path,
        extra_env: &[(&str, &str)],
    ) -> anyhow::Result<(Terminal, async_channel::Receiver<PtyEvent>)> {
        Terminal::spawn(cwd, GridSize { cols: 80, rows: 24 }, extra_env)
    }

    /// Shows a terminal started with [`TerminalView::spawn`].
    pub fn new(
        terminal: Terminal,
        output: async_channel::Receiver<PtyEvent>,
        cwd: &Path,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn(async move |this, cx| {
            while let Ok(first) = output.recv().await {
                // Everything already waiting is handled in the same frame.
                let mut batch = vec![first];
                while let Ok(next) = output.try_recv() {
                    batch.push(next);
                }
                if this.update(cx, |view, cx| view.handle_output(batch, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            terminal,
            focus: cx.focus_handle(),
            title: None,
            state: AgentState::Shell,
            message: None,
            progress: None,
            cwd: cwd.to_owned(),
            has_program_status: false,
            scroll_remainder: 0.0,
            geometry: Rc::default(),
            drag: Drag::None,
            reported: None,
        }
    }

    fn handle_output(&mut self, batch: Vec<PtyEvent>, cx: &mut Context<Self>) {
        let mut events = Vec::new();
        for item in batch {
            match item {
                PtyEvent::Output(bytes) => events.extend(self.terminal.advance(&bytes)),
                PtyEvent::Exited(_) => {
                    cx.emit(TerminalViewEvent::Exited);
                    return;
                }
            }
        }
        for event in events {
            match event {
                TerminalEvent::Title(title) => {
                    self.title = title;
                    cx.emit(TerminalViewEvent::TitleChanged);
                }
                TerminalEvent::Bell => cx.emit(TerminalViewEvent::Bell),
                TerminalEvent::Copy(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
                TerminalEvent::Status(status) => {
                    self.has_program_status = true;
                    self.state = AgentState::from_program_status(&status);
                    self.message = status.msg;
                    self.progress = status.progress;
                    cx.emit(TerminalViewEvent::StateChanged);
                }
                TerminalEvent::Cwd(path) => {
                    if path != self.cwd {
                        self.cwd = path;
                        cx.emit(TerminalViewEvent::CwdChanged);
                    }
                }
                TerminalEvent::PromptStart => {}
            }
        }
        self.schedule_sync_flush(cx);
        cx.notify();
    }

    /// An open synchronized update must still show if its end marker never arrives.
    fn schedule_sync_flush(&self, cx: &mut Context<Self>) {
        let Some(deadline) = self.terminal.sync_deadline() else { return };
        let wait = deadline.saturating_duration_since(Instant::now());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |view, cx| {
                if view.terminal.sync_deadline().is_some_and(|due| due <= Instant::now()) {
                    view.terminal.flush_sync();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// State reported by a Claude Code hook. Used only when Claude doesn't report its state to the
    /// terminal itself (OSC 7501 is the more precise of the two).
    pub fn hook_state(&mut self, state: AgentState, cx: &mut Context<Self>) {
        if self.has_program_status || self.state == state {
            return;
        }
        self.state = state;
        cx.emit(TerminalViewEvent::StateChanged);
        cx.notify();
    }

    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    pub(crate) fn resize(&mut self, size: GridSize) {
        self.terminal.resize(size);
    }

    /// The program in the foreground, for "close this tab?" questions.
    pub fn foreground_pid(&self) -> Option<i32> {
        self.terminal.foreground_pid()
    }

    pub fn write(&mut self, bytes: impl Into<Vec<u8>>) {
        self.terminal.write(bytes);
    }

    /// Pastes text, wrapped in bracketed-paste markers when the program asked for them.
    pub fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.terminal.mode().contains(TermMode::BRACKETED_PASTE) {
            self.write(format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes());
        } else {
            self.write(text.into_bytes());
        }
    }

    pub fn paste_from_clipboard(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.paste(&text);
        }
    }

    /// Copies the selected text to the clipboard. Returns false when nothing is selected.
    pub fn copy_selection(&self, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.terminal.selected_text() else { return false };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    /// The cell under a window position: row, column, and which half of the cell.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "positions inside a small grid")]
    fn cell_at(&self, position: Point<Pixels>) -> Option<(usize, usize, Side)> {
        let geometry = self.geometry.get()?;
        let x = f32::from(position.x - geometry.origin.x).max(0.0);
        let y = f32::from(position.y - geometry.origin.y).max(0.0);
        let cell_width = f32::from(geometry.cell_width);
        let side = if x % cell_width > cell_width / 2.0 { Side::Right } else { Side::Left };
        Some(((y / f32::from(geometry.line_height)) as usize, (x / cell_width) as usize, side))
    }

    /// Whether mouse events go to the program rather than make a selection. Holding Option (or
    /// Shift) selects anyway, as in other terminals.
    fn program_wants_mouse(&self, modifiers: gpui_kit::Modifiers) -> bool {
        let mode = self.terminal.mode();
        mode.intersects(TermMode::MOUSE_MODE)
            && mode.contains(TermMode::SGR_MOUSE)
            && !modifiers.alt
            && !modifiers.shift
    }

    fn report_mouse(&mut self, row: usize, col: usize, pressed: bool) {
        let kind = if pressed { 'M' } else { 'm' };
        self.terminal.write(format!("\x1b[<0;{};{}{kind}", col + 1, row + 1).into_bytes());
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some((row, col, side)) = self.cell_at(event.position) else { return };
        if self.program_wants_mouse(event.modifiers) {
            self.drag = Drag::Reporting;
            self.reported = Some((row, col));
            self.report_mouse(row, col, true);
        } else {
            self.drag = Drag::Selecting;
            self.terminal.start_selection(row, col, side, event.click_count);
        }
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        // A program that follows drags (Claude Code selecting text) gets each cell the mouse moves
        // to, as a drag of the left button.
        if self.drag == Drag::Reporting {
            let follows = self.terminal.mode().intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION);
            if let Some((row, col, _)) = self.cell_at(event.position)
                && follows
                && self.reported != Some((row, col))
            {
                self.reported = Some((row, col));
                self.terminal.write(format!("\x1b[<32;{};{}M", col + 1, row + 1).into_bytes());
            }
            return;
        }
        if self.drag != Drag::Selecting {
            return;
        }
        if let Some((row, col, side)) = self.cell_at(event.position) {
            self.terminal.extend_selection(row, col, side);
            cx.notify();
        }
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.drag == Drag::Selecting && COPY_ON_SELECT.load(Ordering::Relaxed) {
            self.copy_selection(cx);
        }
        if self.drag == Drag::Reporting
            && let Some((row, col, _)) = self.cell_at(event.position)
        {
            self.report_mouse(row, col, false);
        }
        self.drag = Drag::None;
    }

    pub fn kill(&mut self) {
        self.terminal.kill();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let press = KeyPress {
            key: &keystroke.key,
            text: keystroke.key_char.as_deref(),
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            shift: keystroke.modifiers.shift,
            command: keystroke.modifiers.platform,
            meta: OPTION_AS_META.load(Ordering::Relaxed),
        };
        if let Some(bytes) = keys::encode(&press, self.terminal.mode().contains(TermMode::APP_CURSOR)) {
            self.terminal.clear_selection();
            self.write(bytes);
            cx.stop_propagation();
        }
    }

    /// Sends a key a binding took on its way here, as `key_down` would have.
    fn send_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) {
        let press = KeyPress { key, shift, ..KeyPress::default() };
        if let Some(bytes) = keys::encode(&press, self.terminal.mode().contains(TermMode::APP_CURSOR)) {
            self.terminal.clear_selection();
            self.write(bytes);
        }
        cx.notify();
    }

    fn scroll_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let lines = match event.delta {
            ScrollDelta::Lines(delta) => delta.y,
            ScrollDelta::Pixels(delta) => f32::from(delta.y) / f32::from(window.line_height()),
        } + self.scroll_remainder;
        #[expect(clippy::cast_possible_truncation, reason = "whole lines of a small scroll delta")]
        let whole = lines.trunc() as i32;
        self.scroll_remainder = lines.fract();
        if whole == 0 {
            return;
        }
        let mode = self.terminal.mode();
        if mode.intersects(TermMode::MOUSE_MODE) && mode.contains(TermMode::SGR_MOUSE) {
            // The program wants the wheel itself (Claude's full-screen view does).
            let button = if whole > 0 { 64 } else { 65 };
            let report = format!("\x1b[<{button};1;1M");
            self.terminal.write(report.repeat(whole.unsigned_abs() as usize).into_bytes());
        } else if mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL) {
            let arrow = if whole > 0 { "\x1b[A" } else { "\x1b[B" };
            self.terminal.write(arrow.repeat(whole.unsigned_abs() as usize).into_bytes());
        } else {
            self.terminal.scroll(whole);
        }
        cx.notify();
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        div()
            .size_full()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::key_down))
            .on_action(cx.listener(|this, _: &SendTab, _, cx| this.send_key("tab", false, cx)))
            .on_action(cx.listener(|this, _: &SendBackTab, _, cx| this.send_key("tab", true, cx)))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .child(TerminalElement::new(cx.entity(), focused, self.geometry.clone()))
    }
}
