//! A terminal as a GPUI view: owns the [`Terminal`], turns its output into events for the app,
//! and handles keys, paste and scrolling.

use std::path::{Path, PathBuf};
use std::time::Instant;

use alacritty_terminal::term::TermMode;
use gpui_kit::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, ScrollDelta, ScrollWheelEvent, Styled as _, Window, div,
};
use saturn_core::agent_state::AgentState;

use crate::element::TerminalElement;
use crate::keys::{self, KeyPress};
use crate::pty::PtyEvent;
use crate::terminal::{GridSize, Terminal, TerminalEvent};

/// What a terminal tells the app.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalViewEvent {
    TitleChanged,
    StateChanged,
    Bell,
    /// The shell ended; the tab should close.
    Exited,
}

pub struct TerminalView {
    terminal: Terminal,
    focus: FocusHandle,
    /// The terminal's own title as set by the program.
    pub title: Option<String>,
    pub state: AgentState,
    /// Claude's current activity or question (OSC 7501 `msg`).
    pub message: Option<String>,
    pub cwd: PathBuf,
    scroll_remainder: f32,
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
            cwd: cwd.to_owned(),
            scroll_remainder: 0.0,
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
                TerminalEvent::Status(status) => {
                    self.state = AgentState::from_program_status(&status);
                    self.message = status.msg;
                    cx.emit(TerminalViewEvent::StateChanged);
                }
                TerminalEvent::Cwd(path) => self.cwd = path,
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
        };
        if let Some(bytes) = keys::encode(&press, self.terminal.mode().contains(TermMode::APP_CURSOR)) {
            self.write(bytes);
            cx.stop_propagation();
        }
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
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .child(TerminalElement::new(cx.entity(), focused))
    }
}
