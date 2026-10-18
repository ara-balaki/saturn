//! The window's chrome in the design system. One structure for every container: a panel
//! (`surface-raised`, radius 10, 4px padding) holds a strip of controls and a well (`surface`,
//! radius 6) with the content; controls that belong together sit in a track.

use gpui_kit::{
    AnyElement, App, Bounds, ColorSpace, Corners, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, fill,
    linear_color_stop, linear_gradient, point, prelude::FluentBuilder as _, px, size, svg,
};
use saturn_design::{color, radius, space};

use crate::theme::{self, len};

/// Height of every control: tabs, buttons, track items, segments. Dense, as development tools are.
pub const CONTROL_HEIGHT: f32 = 28.0;

/// A Lucide icon from gpui-kit's bundled set, e.g. `panel-left`. SVGs don't inherit the text
/// colour, so it is given here.
pub fn icon(name: &str, size: f32, color: gpui_kit::Hsla) -> impl IntoElement {
    svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).flex_none().text_color(color)
}

/// The fill of the one primary action: vivid blue sinking toward deep blue, blended in `OKLab`.
fn accent_fill() -> gpui_kit::Background {
    linear_gradient(
        135.,
        linear_color_stop(theme::color(color::BLUE_VIVID), 0.),
        linear_color_stop(theme::color(color::BLUE_VIVID).blend(theme::color(color::BLUE_DEEP).opacity(0.45)), 1.),
    )
    .color_space(ColorSpace::Oklab)
}

/// A panel: the container every region is built from.
pub fn panel() -> gpui_kit::Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .gap(len(space::SPACE_1))
        .size_full()
        .min_w_0()
        .min_h_0()
        .p(len(space::SPACE_1))
        .rounded(len(radius::RADIUS_MD))
        .bg(theme::color(color::SURFACE_RAISED).opacity(theme::GLASS_PANEL))
        .border_1()
        .border_color(theme::color(color::LINE))
}

/// A panel's well, holding its content. 4px inside the panel: 10 - 4 = 6.
pub fn well() -> gpui_kit::Div {
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .rounded(len(radius::RADIUS_SM))
        .bg(theme::color(color::SURFACE).opacity(theme::GLASS_WELL))
        .border_1()
        .border_color(theme::color(color::LINE))
        .overflow_hidden()
}

/// A track: controls that belong together, in one `surface` strip.
pub fn track() -> gpui_kit::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(len(space::SPACE_1))
        .p(len(space::SPACE_1))
        .rounded(len(radius::RADIUS_MD))
        .bg(theme::color(color::SURFACE))
        .border_1()
        .border_color(theme::color(color::LINE))
}

/// An item in a track. Pressed items are raised: selection is tone, not colour.
pub fn track_item(id: impl Into<ElementId>, pressed: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(len(space::SPACE_2))
        .h(len(CONTROL_HEIGHT))
        .min_w(len(CONTROL_HEIGHT))
        .rounded(len(radius::RADIUS_SM))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .map(|item| {
            if pressed {
                item.bg(theme::color(color::CONTROL_PRESSED)).text_color(theme::color(color::INK))
            } else {
                item.text_color(theme::color(color::INK_MUTED)).hover(|s| s.bg(theme::color(color::CONTROL)))
            }
        })
}

/// A thin divider between groups in a track.
pub fn track_separator() -> impl IntoElement {
    div().flex_none().w(px(1.)).h(px(20.)).mx(len(space::SPACE_1)).bg(theme::color(color::LINE_STRONG))
}

/// A square icon button, as tall as every control. Quiet ones have no fill until hovered.
pub fn icon_button(id: impl Into<ElementId>, name: &str, quiet: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(len(CONTROL_HEIGHT))
        .rounded(len(radius::RADIUS_SM))
        .cursor_pointer()
        .map(|button| {
            if quiet {
                button.hover(|s| s.bg(theme::color(color::CONTROL)))
            } else {
                button
                    .bg(theme::color(color::CONTROL))
                    .border_1()
                    .border_color(theme::color(color::LINE))
                    .hover(|s| s.bg(theme::color(color::CONTROL_HOVER)))
            }
        })
        .child(icon(name, 14., theme::color(if quiet { color::INK_MUTED } else { color::INK })))
}

/// A text button with an optional shortcut. `primary` is the one action a view is for: the
/// accent fill with a soft blue bloom beneath it.
pub fn button(
    id: impl Into<ElementId>,
    label: &'static str,
    shortcut: Option<&'static str>,
    primary: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let hint = if primary { theme::color(color::INK_ON_ACCENT).opacity(0.7) } else { theme::color(color::INK_FAINT) };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(len(space::SPACE_3))
        .h(len(CONTROL_HEIGHT))
        .px(len(space::SPACE_4))
        .rounded(len(radius::RADIUS_SM))
        .cursor_pointer()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .map(|button| {
            if primary {
                button.text_color(theme::color(color::INK_ON_ACCENT)).bg(accent_fill()).shadow(vec![
                    gpui_kit::BoxShadow {
                        color: theme::color(color::BLUE_VIVID).opacity(0.35),
                        offset: point(px(0.), px(6.)),
                        blur_radius: px(20.),
                        spread_radius: px(0.),
                        inset: false,
                    },
                ])
            } else {
                button
                    .text_color(theme::color(color::INK))
                    .bg(theme::color(color::CONTROL))
                    .border_1()
                    .border_color(theme::color(color::LINE))
                    .hover(|s| s.bg(theme::color(color::CONTROL_HOVER)))
            }
        })
        .child(label)
        .children(shortcut.map(|keys| div().font_weight(FontWeight::NORMAL).text_color(hint).child(keys)))
}

/// A headline in two weights, the point in semibold: "No terminals **yet**".
pub fn headline(light: &str, bold: &str, size: f32) -> impl IntoElement {
    div()
        .flex()
        .gap(px(size * 0.25))
        .text_size(px(size))
        .line_height(px(size * 1.1))
        .text_color(theme::color(color::INK))
        .child(div().font_weight(FontWeight::LIGHT).child(light.to_owned()))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(bold.to_owned()))
}

/// Frosted glass over the whole window: what's drawn under it, blurred and faintly tinted with the
/// ground. Drawn under an open dialog, so the dialog floats over a frosted window as macOS sheets
/// and popovers do.
pub fn frost() -> impl IntoElement {
    let tint = theme::color(color::GROUND).opacity(0.25);
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| window.paint_backdrop_blur(bounds, Corners::default(), px(18.), tint),
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The dot grid: faint dots on an 18px pitch, only inside empty areas.
pub fn dot_grid() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
            const PITCH: f32 = 18.;
            let dot = theme::color(color::LINE_STRONG);
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let mut y = PITCH / 2.;
            while y < height {
                let mut x = PITCH / 2.;
                while x < width {
                    let origin = point(bounds.origin.x + px(x - 1.), bounds.origin.y + px(y - 1.));
                    window.paint_quad(fill(Bounds::new(origin, size(px(2.), px(2.))), dot).corner_radii(px(1.)));
                    x += PITCH;
                }
                y += PITCH;
            }
        },
    )
    .size_full()
    .absolute()
    .top_0()
    .left_0()
}

/// The sidebar: its header (the window's repos), the Files/Changes switch, an optional strip of
/// controls for the tab, and the tab's content in a well.
pub fn sidebar(
    header: impl IntoElement,
    switch: impl IntoElement,
    strip: Option<AnyElement>,
    content: impl IntoElement,
) -> AnyElement {
    panel().child(header).child(switch).children(strip).child(well().child(content)).into_any_element()
}

/// One repo in the sidebar's list: its name and, below, its branch and folder.
pub fn repo_row(
    id: impl Into<ElementId>,
    name: &str,
    detail: &str,
    selected: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .group("repo")
        .flex()
        .items_center()
        .gap(len(space::SPACE_3))
        .pl(len(space::SPACE_1))
        .pr(len(space::SPACE_1))
        .py(px(2.))
        .rounded(len(radius::RADIUS_SM))
        .cursor_pointer()
        .when(selected, |row| row.bg(theme::color(color::CONTROL)))
        .when(!selected, |row| row.hover(|s| s.bg(theme::color(color::CONTROL))))
        .child(icon_button("repo-icon", "folder", false))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::color(if selected { color::INK } else { color::INK_MUTED }))
                        .truncate()
                        .child(name.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(theme::color(color::INK_FAINT))
                        .truncate()
                        .child(detail.to_owned()),
                ),
        )
}

/// What the grid shows with no terminal open: the dot grid, a headline in two weights, one
/// sentence and the action.
pub fn empty_grid(
    project_name: &str,
    new_terminal: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    panel()
        .child(
            well().child(dot_grid()).child(
                div().absolute().size_full().flex().items_center().justify_center().child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(len(space::SPACE_6))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(len(space::SPACE_3))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme::color(color::INK_FAINT))
                                        .child(project_name.to_owned()),
                                )
                                .child(headline("No terminals", "yet", 44.))
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(theme::color(color::INK_MUTED))
                                        .child("Open a shell in the project folder, then start Claude in it."),
                                ),
                        )
                        .child(button("empty-new", "New terminal", Some("⌘T"), true).on_click(new_terminal)),
                ),
            ),
        )
        .into_any_element()
}

/// How the draggable gaps between panels look: nothing at rest (the gap of ground is the
/// divider), a short blue pill while the pointer is on it or dragging.
pub fn quiet_resize_handle() -> gpui_kit::base::ResizeHandleRenderer {
    std::rc::Rc::new(|handle, _, _| {
        use gpui_kit::base::ResizeHandleState;
        // An empty element, not `None`: `None` brings back the library's own hairline.
        if handle.state() == ResizeHandleState::Idle {
            return Some(div().into_any_element());
        }
        let pill =
            theme::color(color::BLUE).opacity(if handle.state() == ResizeHandleState::Hovered { 0.5 } else { 0.9 });
        // The handle's own area is a one-pixel line; the pill is centred along it and overhangs
        // across it.
        Some(match handle.axis() {
            gpui_kit::Axis::Horizontal => div()
                .flex_none()
                .flex()
                .items_center()
                .w(px(1.))
                .h_full()
                .child(div().flex_none().ml(px(-1.)).w(px(3.)).h(px(36.)).rounded_full().bg(pill))
                .into_any_element(),
            gpui_kit::Axis::Vertical => div()
                .flex_none()
                .flex()
                .justify_center()
                .h(px(1.))
                .w_full()
                .child(div().flex_none().mt(px(-1.)).h(px(3.)).w(px(36.)).rounded_full().bg(pill))
                .into_any_element(),
        })
    })
}
