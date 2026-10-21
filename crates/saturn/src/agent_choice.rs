//! Which agents this Mac has (Claude Code, Codex: installed and logged in), found once as Saturn
//! starts, and which one a new pane starts: the one picked last, remembered in the settings. With
//! only one ready, there's nothing to pick.

use std::time::Duration;

use gpui_kit::{
    App, AppContext as _, Global, IntoElement as _, ParentElement as _, StatefulInteractiveElement as _, Styled as _,
    div, px,
};
use saturn_core::agents::{self, Agent, Found};
use saturn_design::{color, space};

use crate::settings;
use crate::theme::{self, len};

/// What was found, once it has been.
#[derive(Default)]
pub struct Agents {
    pub found: Option<Vec<Found>>,
}

impl Global for Agents {}

/// Finds the agents, off the main thread, once the login shell's `PATH` is known. A spike names
/// them instead (`SATURN_AGENTS=claude,codex`, or empty for none), without running anything.
pub fn start(cx: &mut App) {
    cx.set_global(Agents::default());
    if let Ok(names) = std::env::var("SATURN_AGENTS") {
        let named: Vec<Agent> = names.split(',').filter_map(|name| Agent::of_program(name.trim())).collect();
        let found = Agent::ALL
            .into_iter()
            .map(|agent| Found { agent, installed: named.contains(&agent), logged_in: named.contains(&agent) })
            .collect();
        cx.global_mut::<Agents>().found = Some(found);
        return;
    }
    cx.spawn(async move |cx| {
        // The login shell's PATH is read as Saturn starts; it can take a moment.
        for _ in 0..50 {
            if crate::LOGIN_PATH.get().is_some() {
                break;
            }
            cx.background_executor().timer(Duration::from_millis(100)).await;
        }
        let path = crate::LOGIN_PATH
            .get()
            .cloned()
            .unwrap_or_else(|| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_owned());
        let found = cx.background_spawn(async move { agents::detect(&path) }).await;
        cx.update(|cx| {
            cx.global_mut::<Agents>().found = Some(found);
            cx.refresh_windows();
        });
    })
    .detach();
}

/// The agents that can start: installed and logged in. Before they're found, Claude Code, as
/// Saturn has always started.
pub fn ready(cx: &App) -> Vec<Agent> {
    match cx.try_global::<Agents>().and_then(|agents| agents.found.as_ref()) {
        Some(found) => found.iter().filter(|found| found.ready()).map(|found| found.agent).collect(),
        None => vec![Agent::Claude],
    }
}

/// The agent a new pane starts: the one picked last when it's ready, else the first ready one.
pub fn current(cx: &App) -> Agent {
    let last = settings::load().agent;
    match cx.try_global::<Agents>().and_then(|agents| agents.found.as_ref()) {
        Some(found) => agents::pick(found, last),
        None => last.unwrap_or(Agent::Claude),
    }
}

/// Remembers `agent` as the one to start next time.
pub fn remember(agent: Agent) {
    let mut settings = settings::load();
    if settings.agent != Some(agent) {
        settings.agent = Some(agent);
        settings::save(&settings);
    }
}

/// The Claude Code | Codex switch, with what it's for (`label`): picking one remembers it for every
/// later start. `None` with fewer than two agents ready, when there's nothing to pick.
pub fn row(id: &'static str, label: &'static str, cx: &App) -> Option<gpui_kit::AnyElement> {
    let track = switch(id, cx)?;
    Some(
        div()
            .flex()
            .items_center()
            .gap(len(space::SPACE_2))
            .pt(len(space::SPACE_2))
            .child(div().text_size(px(13.)).text_color(theme::color(color::INK_MUTED)).child(label))
            .child(track)
            .into_any_element(),
    )
}

/// The switch alone: a track with one item per ready agent, the current one raised.
pub fn switch(id: &'static str, cx: &App) -> Option<gpui_kit::AnyElement> {
    let ready = ready(cx);
    if ready.len() < 2 {
        return None;
    }
    let current = current(cx);
    let mut track = crate::chrome::track();
    for agent in ready {
        track = track.child(
            crate::chrome::track_item((id, agent as usize), agent == current)
                .px(len(space::SPACE_3))
                .text_size(px(12.))
                .child(agent.name())
                .on_click(move |_, window, _| {
                    remember(agent);
                    window.refresh();
                }),
        );
    }
    Some(track.into_any_element())
}
