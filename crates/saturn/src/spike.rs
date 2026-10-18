//! A harness for performance spikes and screenshots, driven by environment variables:
//!
//! - `SATURN_PANES=N` opens N panes, `SATURN_COMMAND` types a command into each.
//! - `SATURN_PERF=1` prints how long terminal frames take, every second.
//! - `SATURN_SCREENSHOT=path.png` saves what the window draws after `SATURN_SCREENSHOT_AFTER`
//!   seconds (needs the `capture` feature); `SATURN_QUIT_AFTER` quits after that many seconds.

use std::time::Duration;

use gpui_kit::{Context, Window};
use saturn_core::layout::TabId;

use crate::workspace::{Placement, Workspace};

fn var<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|value| value.parse().ok())
}

pub fn panes() -> usize {
    var("SATURN_PANES").unwrap_or(0)
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
        let tabs: Vec<TabId> = (1..=panes() as u64).map(TabId).collect();
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
fn capture(window: &mut Window, cx: &mut Context<Workspace>) {
    let Ok(path) = std::env::var("SATURN_SCREENSHOT") else { return };
    let after = var::<u64>("SATURN_SCREENSHOT_AFTER").unwrap_or(3);
    let handle = window.window_handle();
    cx.spawn(async move |_, cx| {
        cx.background_executor().timer(Duration::from_secs(after)).await;
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
fn capture(_: &mut Window, _: &mut Context<Workspace>) {}
