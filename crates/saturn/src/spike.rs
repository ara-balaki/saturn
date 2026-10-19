//! A harness for performance spikes and screenshots, driven by environment variables:
//!
//! - `SATURN_PANES=N` opens N panes instead of one shell (0: the empty grid), `SATURN_COMMAND`
//!   types a command into each.
//! - `SATURN_PERF=1` prints how long terminal frames take, every second.
//! - `SATURN_OPEN_SETTINGS=1` opens the Settings dialog on start; `SATURN_OPEN_FILE=path` opens a file
//!   in the editor (`SATURN_SAVE=1` saves it after a moment); `SATURN_REVIEW=1` shows the Changes tab and reviews the first change.
//! - `SATURN_MORE_REPOS=a:b` adds more repos to the window, as ⌘O would; `SATURN_FAKE_AGENTS=1`
//!   gives the panes made-up agents; `SATURN_HISTORY=1` opens the History tab, `SATURN_IMPACT=1` the Impact tab (focused on `SATURN_IMPACT_FOCUS`), `SATURN_SCHEMA=1`
//!   the database diagram; `SATURN_LSP_PROBE=line:col` asks the open file's language server about
//!   that place (0-based) and prints its answers; `SATURN_PROBLEMS=1` lists the
//!   servers' errors and warnings; `SATURN_PALETTE=files|commands` opens a palette, with
//!   `SATURN_PALETTE_QUERY` typed in it.
//! - `SATURN_SCREENSHOT=path.png` saves what the window draws after `SATURN_SCREENSHOT_AFTER`
//!   seconds (needs the `capture` feature); `SATURN_QUIT_AFTER` quits after that many seconds.

use std::time::Duration;

use gpui_kit::{Context, Window};
use saturn_core::layout::TabId;

use crate::workspace::{Placement, Workspace};

fn var<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|value| value.parse().ok())
}

/// Whether this run is driven by a script (screenshot or timed quit) rather than a person.
pub fn is_automated() -> bool {
    std::env::var_os("SATURN_SCREENSHOT").is_some() || std::env::var_os("SATURN_QUIT_AFTER").is_some()
}

/// How many panes to open on start, when a spike asks for a number.
pub fn panes() -> Option<usize> {
    var("SATURN_PANES")
}

/// Lays N panes out like the Electron spike: one row up to 3, two up to 8, three beyond.
pub fn placement(index: usize, panes: usize) -> Placement {
    let rows = match panes {
        0..=3 => 1,
        4..=8 => 2,
        _ => 3,
    };
    let per_row = panes.div_ceil(rows);
    match index {
        0 => Placement::Tab,
        i if i % per_row == 0 => Placement::Down,
        _ => Placement::Right,
    }
}

pub fn start(workspace: &Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    if let Ok(command) = std::env::var("SATURN_COMMAND") {
        let tabs: Vec<TabId> = (1..=panes().unwrap_or(0) as u64).map(TabId).collect();
        let views: Vec<_> = tabs.iter().filter_map(|tab| workspace.terminal(*tab).cloned()).collect();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            for view in views {
                let () = view.update(cx, |view, _| view.write(format!("{command}\r").into_bytes()));
            }
        })
        .detach();
    }
    if std::env::var("SATURN_PERF").is_ok() {
        cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let (frames, spent) = saturn_terminal::element::stats::take();
                eprintln!("perf: {frames} terminal frames, {:.1} ms laying them out", spent.as_secs_f64() * 1000.0);
            }
        })
        .detach();
    }
    if std::env::var_os("SATURN_OPEN_SETTINGS").is_some() {
        // After the window (and the Root that hosts dialogs) exists.
        cx.defer_in(window, Workspace::open_settings);
    }
    if std::env::var_os("SATURN_SCHEMA").is_some() {
        cx.defer_in(window, |workspace, _, cx| workspace.show_schema(cx));
    }
    if std::env::var_os("SATURN_IMPACT").is_some() {
        cx.defer_in(window, |workspace, _, cx| {
            let root = workspace.root().to_owned();
            workspace.show_impact(&root, cx);
        });
    }
    if std::env::var_os("SATURN_HISTORY").is_some() {
        cx.defer_in(window, |workspace, _, cx| workspace.show_history(cx));
    }
    if std::env::var_os("SATURN_FAKE_TICKETS").is_some() {
        cx.defer_in(window, Workspace::fake_tickets);
    }
    if std::env::var_os("SATURN_FAKE_AGENTS").is_some() {
        cx.defer_in(window, |workspace, _, cx| workspace.fake_agents(cx));
    }
    if std::env::var_os("SATURN_REVIEW").is_some() {
        cx.defer_in(window, |workspace, _, cx| workspace.review_first_change(cx));
    }
    if std::env::var_os("SATURN_SAVE").is_some() {
        // After SATURN_OPEN_FILE has opened its file.
        cx.spawn(async move |workspace, cx| {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            let _ = workspace.update(cx, Workspace::save_file);
        })
        .detach();
    }
    if let Some(paths) = std::env::var_os("SATURN_MORE_REPOS") {
        // Repos added after the first, `:`-separated, as ⌘O would add them.
        for path in std::env::split_paths(&paths) {
            cx.defer_in(window, move |workspace, window, cx| workspace.add_project(path, window, cx));
        }
        // The first repo stays selected, as if the others were added earlier.
        let first = workspace.project_path().to_owned();
        cx.defer_in(window, move |workspace, window, cx| workspace.select_project(first, window, cx));
    }
    if let Some((line, character)) =
        std::env::var("SATURN_LSP_PROBE").ok().and_then(|at| at.split_once(':').map(|(l, c)| (l.parse(), c.parse())))
        && let (Ok(line), Ok(character)) = (line, character)
    {
        // After the opened file's server has had a moment to start.
        cx.spawn_in(window, async move |workspace, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ =
                workspace.update_in(cx, |workspace, window, cx| workspace.probe_language(line, character, window, cx));
        })
        .detach();
    }
    if let Ok(which) = std::env::var("SATURN_PALETTE") {
        open_palette(which, window, cx);
    }
    if std::env::var_os("SATURN_MAXIMIZE_EDITOR").is_some() {
        cx.defer_in(window, |workspace, _, cx| workspace.maximize_editor(cx));
    }
    if std::env::var_os("SATURN_PROBLEMS").is_some() {
        // Once the opened file's server has reported.
        cx.spawn(async move |workspace, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = workspace.update(cx, Workspace::show_problems);
        })
        .detach();
    }
    if let Some(path) = std::env::var_os("SATURN_OPEN_FILE") {
        cx.defer_in(window, move |workspace, window, cx| workspace.open_file(std::path::Path::new(&path), window, cx));
    }
}

/// Opens a palette (`files` or `commands`) once the window is up, and reports whether its search
/// has the keyboard.
fn open_palette(which: String, window: &mut Window, cx: &mut Context<Workspace>) {
    // After the window and its dialog layer exist: `files` (⌘P) or `commands` (⌘⇧P).
    cx.spawn_in(window, async move |workspace, cx| {
        cx.background_executor().timer(Duration::from_millis(800)).await;
        let _ = workspace.update_in(cx, |workspace, window, cx| {
            if which == "files" {
                crate::palette::go_to_file(cx.weak_entity(), workspace.root().to_owned(), window, cx);
            } else {
                crate::palette::commands(window, cx);
            }
        });
        cx.background_executor().timer(Duration::from_millis(600)).await;
        let _ = workspace.update_in(cx, |_, window, cx| {
            let palette =
                crate::palette::LAST.with_borrow(|last| last.as_ref().and_then(gpui_kit::WeakEntity::upgrade));
            if let Some(palette) = palette {
                eprintln!("palette: search focused = {}", palette.read(cx).search_has_focus(window, cx));
            }
        });
    })
    .detach();
}

/// The screenshot and the timed quit, for the window itself: they apply on the welcome screen too.
pub fn start_window<T: 'static>(window: &mut Window, cx: &mut Context<T>) {
    capture(window, cx);
    if let Some(seconds) = var::<u64>("SATURN_QUIT_AFTER") {
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(Duration::from_secs(seconds)).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}

#[cfg(feature = "capture")]
fn capture<T: 'static>(window: &mut Window, cx: &mut Context<T>) {
    let Ok(path) = std::env::var("SATURN_SCREENSHOT") else { return };
    let after = var::<u64>("SATURN_SCREENSHOT_AFTER").unwrap_or(3);
    let handle = window.window_handle();
    cx.spawn(async move |_, cx| {
        cx.background_executor().timer(Duration::from_secs(after)).await;
        // macOS stops sending frames to a window it considers hidden, so draw one now: what
        // arrived late (a shell's output, a server's answer) is then in the picture.
        let _ = handle.update(cx, |_, window, cx| window.draw(cx).clear(cx));
        let _ = handle.update(cx, |_, window, _| match window.render_to_image() {
            Ok(image) => match image.save(&path) {
                Ok(()) => eprintln!("screenshot: {path}"),
                Err(error) => eprintln!("screenshot failed: {error}"),
            },
            Err(error) => eprintln!("screenshot failed: {error}"),
        });
    })
    .detach();
}

#[cfg(not(feature = "capture"))]
fn capture<T: 'static>(_: &mut Window, _: &mut Context<T>) {}
