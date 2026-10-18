//! One terminal: a shell in a pty, alacritty's engine keeping the screen, and the signal tap for
//! what alacritty doesn't read.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use alacritty_terminal::event::{Event as EngineEvent, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::{Config, Term, TermMode, viewport_to_point};
use alacritty_terminal::vte::ansi::Processor;
use saturn_core::program_status::{PROBE_REPLY, ProgramStatus};

use crate::pty::{Pty, PtyEvent};
use crate::signals::{Signal, SignalTap};

pub const SCROLLBACK_LINES: usize = 10_000;

/// What happened in a terminal that the app cares about.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalEvent {
    /// The terminal title changed; `None` when the program reset it.
    Title(Option<String>),
    Bell,
    /// Claude Code reported its state (OSC 7501).
    Status(ProgramStatus),
    Cwd(PathBuf),
    PromptStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        usize::from(self.rows)
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.rows)
    }

    fn columns(&self) -> usize {
        usize::from(self.cols)
    }
}

/// Collects alacritty's events while it parses; drained right after.
#[derive(Clone, Default)]
pub struct EngineEvents(Arc<Mutex<Vec<EngineEvent>>>);

impl EventListener for EngineEvents {
    fn send_event(&self, event: EngineEvent) {
        if let Ok(mut events) = self.0.lock() {
            events.push(event);
        }
    }
}

impl EngineEvents {
    fn drain(&self) -> Vec<EngineEvent> {
        self.0.lock().map(|mut events| std::mem::take(&mut *events)).unwrap_or_default()
    }
}

pub struct Terminal {
    term: Term<EngineEvents>,
    processor: Processor,
    tap: SignalTap,
    events: EngineEvents,
    pty: Pty,
    size: GridSize,
}

impl Terminal {
    /// Starts a shell in `cwd`. The receiver delivers its output for [`Terminal::advance`].
    pub fn spawn(
        cwd: &Path,
        size: GridSize,
        extra_env: &[(&str, &str)],
    ) -> anyhow::Result<(Self, async_channel::Receiver<PtyEvent>)> {
        let (pty, output) = Pty::spawn(cwd, size.cols, size.rows, extra_env)?;
        let events = EngineEvents::default();
        let config = Config { scrolling_history: SCROLLBACK_LINES, ..Config::default() };
        let term = Term::new(config, &size, events.clone());
        Ok((Self { term, processor: Processor::new(), tap: SignalTap::default(), events, pty, size }, output))
    }

    /// Feeds program output to the screen and returns what the app should know about.
    pub fn advance(&mut self, bytes: &[u8]) -> Vec<TerminalEvent> {
        let mut out = Vec::new();
        for signal in self.tap.advance(bytes) {
            match signal {
                Signal::StatusProbe => self.pty.write(PROBE_REPLY),
                Signal::Status(status) => out.push(TerminalEvent::Status(status)),
                Signal::Cwd(path) => out.push(TerminalEvent::Cwd(path)),
                Signal::PromptStart => out.push(TerminalEvent::PromptStart),
            }
        }
        self.processor.advance(&mut self.term, bytes);
        for event in self.events.drain() {
            match event {
                // Replies the program asked for: device attributes, cursor position…
                EngineEvent::PtyWrite(reply) => self.pty.write(reply.into_bytes()),
                EngineEvent::Title(title) => out.push(TerminalEvent::Title(Some(title))),
                EngineEvent::ResetTitle => out.push(TerminalEvent::Title(None)),
                EngineEvent::Bell => out.push(TerminalEvent::Bell),
                _ => {}
            }
        }
        out
    }

    /// When a synchronized update (DEC 2026) is open: the moment to show it even if its end never
    /// arrives.
    pub fn sync_deadline(&self) -> Option<Instant> {
        self.processor.sync_timeout().sync_timeout()
    }

    /// Shows a synchronized update whose end marker did not arrive in time.
    pub fn flush_sync(&mut self) {
        self.processor.stop_sync(&mut self.term);
    }

    pub fn term(&self) -> &Term<EngineEvents> {
        &self.term
    }

    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    pub fn size(&self) -> GridSize {
        self.size
    }

    pub fn resize(&mut self, size: GridSize) {
        if size == self.size || size.cols == 0 || size.rows == 0 {
            return;
        }
        self.size = size;
        self.term.resize(size);
        self.pty.resize(size.cols, size.rows);
    }

    pub fn write(&mut self, bytes: impl Into<Vec<u8>>) {
        // Typing jumps back to the live screen, as in every terminal.
        if self.term.grid().display_offset() != 0 {
            self.term.scroll_display(Scroll::Bottom);
        }
        self.pty.write(bytes);
    }

    /// Scrolls the scrollback by whole lines; positive is up.
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    /// The grid point for a cell on screen (row and column counted from the top-left of the view).
    fn grid_point(&self, row: usize, col: usize) -> Point {
        let row = row.min(self.term.screen_lines().saturating_sub(1));
        let col = col.min(self.term.columns().saturating_sub(1));
        viewport_to_point(self.term.grid().display_offset(), Point::new(row, Column(col)))
    }

    /// Starts selecting at a cell: by character, by word (double click) or by line (triple click).
    pub fn start_selection(&mut self, row: usize, col: usize, side: Side, clicks: usize) {
        let kind = match clicks {
            2 => SelectionType::Semantic,
            3.. => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        self.term.selection = Some(Selection::new(kind, self.grid_point(row, col), side));
    }

    pub fn extend_selection(&mut self, row: usize, col: usize, side: Side) {
        let point = self.grid_point(row, col);
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// The selected text, or `None` when nothing (or only empty space) is selected.
    pub fn selected_text(&self) -> Option<String> {
        self.term.selection_to_string().filter(|text| !text.trim().is_empty())
    }

    pub fn foreground_pid(&self) -> Option<i32> {
        self.pty.foreground_pid()
    }

    pub fn kill(&mut self) {
        self.pty.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_and_copies_screen_text() {
        let size = GridSize { cols: 40, rows: 5 };
        let (mut terminal, _output) = Terminal::spawn(Path::new("/tmp"), size, &[]).expect("a shell starts");
        terminal.advance(b"\x1b[2J\x1b[Hfirst line\r\nsecond word here");
        terminal.start_selection(1, 7, Side::Left, 2);
        assert_eq!(terminal.selected_text().as_deref(), Some("word"));
        terminal.start_selection(0, 0, Side::Left, 1);
        terminal.extend_selection(1, 5, Side::Right);
        assert_eq!(terminal.selected_text().as_deref(), Some("first line\nsecond"));
        terminal.clear_selection();
        assert_eq!(terminal.selected_text(), None);
    }

    #[test]
    fn runs_a_shell_and_reads_its_output_and_signals() {
        let size = GridSize { cols: 80, rows: 24 };
        let (mut terminal, output) =
            Terminal::spawn(Path::new("/tmp"), size, &[("TERM", "xterm-256color")]).expect("a shell starts");
        terminal.write(
            b"printf '\\033]0;ABC-236\\007\\033]7501;state=done:app=claude-code\\007'; echo saturn-$((40+2)); exit\r"
                .to_vec(),
        );

        let mut events = Vec::new();
        let mut screen = String::new();
        while let Ok(event) = output.recv_blocking() {
            match event {
                PtyEvent::Output(bytes) => events.extend(terminal.advance(&bytes)),
                PtyEvent::Exited(_) => break,
            }
        }
        for line in 0..terminal.term().screen_lines() {
            let row = &terminal.term().grid()[alacritty_terminal::index::Line(i32::try_from(line).expect("small"))];
            screen.extend(row.into_iter().map(|cell| cell.c));
        }
        assert!(screen.contains("saturn-42"), "screen: {screen}");
        assert!(events.contains(&TerminalEvent::Title(Some("ABC-236".into()))), "events: {events:?}");
        assert!(
            events.iter().any(|e| matches!(e, TerminalEvent::Status(s) if s.state == "done")),
            "events: {events:?}"
        );
    }
}
