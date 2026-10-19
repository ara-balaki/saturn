//! The Claude plan's usage (`/usage`: the session, the week, a model's week), read for the whole
//! app and shown at the foot of every window's sidebar. Read every few minutes and soon after an
//! agent finishes a turn; hidden when Claude Code has no plan limits to report.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui_kit::{
    App, AppContext as _, FontWeight, Global, Hsla, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, px,
};
use saturn_core::agent_state::AgentState;
use saturn_core::usage::{self, Limit};
use saturn_design::space;

use crate::chrome;
use crate::theme::len;

/// How often the usage is read while nothing happens.
const EVERY: Duration = Duration::from_mins(5);
/// The least time between two reads, however many turns end.
const AT_MOST_EVERY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct Usage {
    pub limits: Option<Vec<Limit>>,
    read_at: Option<Instant>,
    reading: bool,
}

impl Global for Usage {}

/// Starts reading the usage, once for the app. Automated runs show made-up numbers when asked
/// (`SATURN_FAKE_USAGE`), and never start `claude` on their own.
pub fn start(cx: &mut App) {
    cx.set_global(Usage::default());
    if crate::spike::is_automated() || std::env::var_os("SATURN_FAKE_USAGE").is_some() {
        if std::env::var_os("SATURN_FAKE_USAGE").is_some() {
            cx.global_mut::<Usage>().limits = Some(made_up());
        }
        return;
    }
    cx.spawn(async move |cx| {
        // The login shell's PATH (where `claude` lives) is read as Saturn starts.
        cx.background_executor().timer(Duration::from_secs(3)).await;
        loop {
            cx.update(|cx| refresh(false, cx));
            cx.background_executor().timer(EVERY).await;
        }
    })
    .detach();
}

/// Reads the usage again: `now` when asked by hand, else not within a minute of the last read.
pub fn refresh(now: bool, cx: &mut App) {
    let Some(usage) = cx.try_global::<Usage>() else { return };
    let recent = usage.read_at.is_some_and(|at| at.elapsed() < AT_MOST_EVERY);
    if usage.reading || (recent && !now) || std::env::var_os("SATURN_FAKE_USAGE").is_some() {
        return;
    }
    cx.global_mut::<Usage>().reading = true;
    let path = crate::lsp::login_path();
    cx.spawn(async move |cx| {
        let limits = cx.background_spawn(async move { usage::read(&path) }).await;
        cx.update(|cx| {
            let usage = cx.global_mut::<Usage>();
            usage.reading = false;
            usage.read_at = Some(Instant::now());
            usage.limits = limits;
            cx.refresh_windows();
        });
    })
    .detach();
}

/// The demo's numbers.
fn made_up() -> Vec<Limit> {
    let now = unix_now();
    vec![
        Limit { label: "Session".to_owned(), percent: 34., resets_at: Some(now + 2 * 3600 + 900) },
        Limit { label: "Week".to_owned(), percent: 61., resets_at: Some(now + 4 * 86_400) },
        Limit { label: "Fable".to_owned(), percent: 12., resets_at: Some(now + 4 * 86_400) },
    ]
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(0))
}

/// White at `alpha`: the gradient card always carries white text.
fn white(alpha: f32) -> Hsla {
    gpui_kit::hsla(0., 0., 1., alpha)
}

/// A hairline meter on the gradient, white with a soft glow; orange from 80%.
fn meter(percent: f64) -> impl IntoElement {
    #[expect(clippy::cast_possible_truncation, reason = "a share between 0 and 1")]
    let width = (percent / 100.).clamp(0., 1.) as f32;
    let fill = if percent >= 80. { gpui_kit::Hsla::from(gpui_kit::rgb(0xff_b3_7a)) } else { white(1.) };
    div().relative().h(px(2.)).w_full().rounded(px(1.)).bg(white(0.18)).child(
        div().absolute().top_0().left_0().h_full().w(gpui_kit::relative(width)).rounded(px(1.)).bg(fill).shadow(vec![
            gpui_kit::BoxShadow {
                color: fill.opacity(0.6),
                offset: gpui_kit::point(px(0.), px(0.)),
                blur_radius: px(6.),
                spread_radius: px(0.),
                inset: false,
            },
        ]),
    )
}

/// The plan's limits as a card at the foot of the sidebar, on the same gradient as an agent's.
/// Clicking it reads them again.
pub fn render(cx: &App) -> Option<impl IntoElement> {
    let limits = cx.try_global::<Usage>()?.limits.clone()?;
    let now = unix_now();
    let resets = |limit: &Limit| limit.resets_at.map(|at| usage::resets_in(at, now)).unwrap_or_default();
    let card = crate::agents_view::surface("usage", AgentState::Working)
        .gap(len(space::SPACE_3))
        .cursor_pointer()
        .tooltip(chrome::tip("Your Claude plan's limits, as /usage shows them. Click to read them again"))
        .on_click(|_, _, cx| refresh(true, cx));
    let mut rows = div().relative().flex().flex_col().gap(len(space::SPACE_3));
    for limit in &limits {
        rows = rows.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(len(space::SPACE_2))
                        .text_size(px(13.))
                        .text_color(white(1.))
                        .child(limit.label.clone())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(white(0.5))
                                .child(format!("resets {}", resets(limit))),
                        )
                        .child(div().font_weight(FontWeight::MEDIUM).child(format!("{:.0}%", limit.percent))),
                )
                .child(meter(limit.percent)),
        );
    }
    let title = div().relative().text_size(px(12.)).text_color(white(0.62)).child("Usage");
    Some(card.child(title).child(rows))
}
