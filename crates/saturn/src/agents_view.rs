//! The Agents tab: a tile per agent. The agent in the focused pane gets the gradient card, the
//! one coloured surface on a screen, tinted by its state; the others are compact cards whose icon
//! tile lights up (the accent tile) while they work.

use gpui_kit::{
    AnyElement, ColorSpace, ElementId, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _, div, linear_color_stop, linear_gradient, point, prelude::FluentBuilder as _, px, rgb,
};
use saturn_core::agent_activity::{Steps, short_tokens};
use saturn_core::agent_state::AgentState;
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};

/// What a tile shows about one agent.
pub struct Agent {
    pub name: String,
    pub state: AgentState,
    /// Claude's current activity or question.
    pub message: Option<String>,
    /// 0–100, when Claude reports it.
    pub progress: Option<u8>,
    pub steps: Option<Steps>,
    /// Tokens in the session's context.
    pub context: Option<u64>,
}

impl Agent {
    fn label(&self) -> String {
        match (&self.steps, self.state) {
            (Some(steps), AgentState::Working) => {
                format!("Step {} of {}", (steps.done + 1).min(steps.total), steps.total)
            }
            (_, AgentState::NeedsYou) => "Needs you".to_owned(),
            (_, AgentState::YourTurn) => "Your turn".to_owned(),
            (_, AgentState::Error) => "Stopped".to_owned(),
            (_, AgentState::Working) => "Working".to_owned(),
            _ => "Idle".to_owned(),
        }
    }

    /// What it's doing now: the step in progress, else Claude's own words.
    fn activity(&self) -> Option<String> {
        self.steps.as_ref().and_then(|steps| steps.current.clone()).or_else(|| self.message.clone())
    }

    /// How far it is, 0–1: Claude's progress, else the share of finished steps.
    fn fraction(&self) -> Option<f32> {
        self.progress.map(|progress| f32::from(progress) / 100.).or_else(|| {
            #[expect(clippy::cast_precision_loss, reason = "step counts are small")]
            self.steps.as_ref().filter(|steps| steps.total > 0).map(|steps| steps.done as f32 / steps.total as f32)
        })
    }
}

/// The gradient's bright and deep ends for a state. The card always carries white text, so these
/// are the same in light and dark mode.
fn tint(state: AgentState) -> (u32, u32, u32) {
    match state {
        AgentState::NeedsYou | AgentState::Error => (0xff_8a_3d, 0x4a_1d_05, 0x0d_06_03),
        AgentState::YourTurn => (0x2f_9e_57, 0x0b_3a_1c, 0x03_0b_06),
        _ => (0x1c_69_e3, 0x07_28_69, 0x04_08_14),
    }
}

fn state_color(state: AgentState) -> Hsla {
    theme::color(match state {
        AgentState::Working => color::STATUS_WORKING,
        AgentState::NeedsYou | AgentState::Error => color::STATUS_NEEDS_YOU,
        AgentState::YourTurn => color::STATUS_YOUR_TURN,
        _ => color::STATUS_IDLE,
    })
}

/// The gradient card for the agent in the focused pane.
pub fn focused_tile(id: impl Into<ElementId>, agent: &Agent) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (bright, deep, end) = tint(agent.state);
    let white = |alpha: f32| gpui_kit::hsla(0., 0., 1., alpha);
    div()
        .id(id)
        .relative()
        .flex()
        .flex_col()
        .gap(len(space::SPACE_1))
        .p(len(space::SPACE_3))
        .rounded(len(radius::RADIUS_LG))
        .overflow_hidden()
        .cursor_pointer()
        .bg(linear_gradient(155., linear_color_stop(rgb(deep), 0.), linear_color_stop(rgb(end), 1.))
            .color_space(ColorSpace::Oklab))
        // The light pooling in from the lower left, as in the design system's gradient card.
        .child(
            div().absolute().top_0().left_0().size_full().bg(linear_gradient(
                35.,
                linear_color_stop(Hsla::from(rgb(bright)).opacity(0.85), 0.),
                linear_color_stop(Hsla::from(rgb(bright)).opacity(0.), 0.6),
            )
            .color_space(ColorSpace::Oklab)),
        )
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(12.))
                .text_color(white(0.62))
                .child(agent.label())
                .children(agent.context.map(|tokens| {
                    div()
                        .flex()
                        .items_center()
                        .gap(len(space::SPACE_1))
                        .text_color(white(0.9))
                        .child(chrome::icon("chart-no-axes-column", 12., white(0.9)))
                        .child(format!("Context {}", short_tokens(tokens)))
                })),
        )
        .child(
            div()
                .relative()
                .text_size(px(15.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(white(1.))
                .truncate()
                .child(agent.name.clone()),
        )
        .children(
            agent
                .activity()
                .map(|activity| div().relative().text_size(px(12.)).text_color(white(0.72)).truncate().child(activity)),
        )
        .children(agent.fraction().map(|fraction| meter(fraction, white)))
}

/// The darker glass well with a hairline meter and a soft glow at its end, like Quarn's progress line.
fn meter(fraction: f32, white: impl Fn(f32) -> Hsla) -> impl IntoElement {
    let percent = (fraction.clamp(0., 1.) * 100.).round();
    div()
        .relative()
        .mt(len(space::SPACE_2))
        .p(len(space::SPACE_3))
        .flex()
        .flex_col()
        .gap(len(space::SPACE_2))
        .rounded(len(radius::RADIUS_SM))
        .bg(gpui_kit::hsla(0., 0., 0., 0.28))
        .border_1()
        .border_color(white(0.06))
        .child(
            div().relative().h(px(2.)).w_full().rounded(px(1.)).bg(white(0.18)).child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(gpui_kit::relative(fraction.clamp(0., 1.)))
                    .rounded(px(1.))
                    .bg(white(1.))
                    .shadow(vec![gpui_kit::BoxShadow {
                        color: white(0.6),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(6.),
                        spread_radius: px(0.),
                        inset: false,
                    }]),
            ),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(11.))
                .text_color(white(0.55))
                .child("0%")
                .child(div().text_color(white(0.9)).child(format!("{percent}%")))
                .child("100%"),
        )
}

/// A compact card for an agent in another pane.
pub fn tile(id: impl Into<ElementId>, agent: &Agent) -> gpui_kit::Stateful<gpui_kit::Div> {
    let working = agent.state == AgentState::Working;
    let (bright, deep, _) = tint(AgentState::Working);
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(len(space::SPACE_3))
        .p(len(space::SPACE_2))
        .rounded(len(radius::RADIUS_MD))
        .cursor_pointer()
        .bg(theme::overlay(0.05))
        .hover(|s| s.bg(theme::fill_control()))
        // The accent tile while it works; otherwise a quiet tile with the state's colour.
        .child(
            div().flex().flex_none().items_center().justify_center().size(px(28.)).rounded(len(radius::RADIUS_SM)).map(
                |icon| {
                    if working {
                        icon.bg(linear_gradient(
                            135.,
                            linear_color_stop(rgb(bright), 0.),
                            linear_color_stop(rgb(deep), 1.),
                        )
                        .color_space(ColorSpace::Oklab))
                            .child(chrome::icon("bot", 14., gpui_kit::white()))
                    } else {
                        icon.bg(theme::fill_control()).child(chrome::icon("bot", 14., state_color(agent.state)))
                    }
                },
            ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::color(color::INK))
                        .truncate()
                        .child(agent.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme::color(color::INK_FAINT))
                        .truncate()
                        .child(agent.activity().unwrap_or_else(|| agent.label())),
                ),
        )
        .children(agent.context.map(|tokens| {
            div().flex_none().text_size(px(11.)).text_color(theme::color(color::INK_FAINT)).child(short_tokens(tokens))
        }))
}

/// What the tab says with no agents yet.
pub fn empty() -> AnyElement {
    div()
        .p(len(space::SPACE_3))
        .text_size(px(13.))
        .text_color(theme::color(color::INK_MUTED))
        .child("No agents yet. Start Claude in a pane and it shows up here.")
        .into_any_element()
}
