//! The window's chrome in the design system: icon tiles, buttons, pills, the textures, and the
//! sidebar, editor and empty-grid placeholders.

use gpui_kit::{
    AnyElement, App, Bounds, ColorSpace, ElementId, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _, Styled as _, StyledText, Window, canvas,
    div, fill, linear_color_stop, linear_gradient, pattern_slash, point, prelude::FluentBuilder as _, px, size, svg,
};
use saturn_design::{color, radius, shadow, space};

use crate::theme::{self, len};

/// A Lucide icon from gpui-kit's bundled set, e.g. `panel-left`. SVGs don't inherit the text
/// colour, so it is given here.
pub fn icon(name: &str, size: f32, color: gpui_kit::Hsla) -> impl IntoElement {
    svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).flex_none().text_color(color)
}

/// The brand gradient (orange → purple, blended in `OKLab`), for what marks where you are.
pub fn brand_gradient() -> gpui_kit::Background {
    linear_gradient(
        135.,
        linear_color_stop(theme::color(color::ORANGE), 0.),
        linear_color_stop(theme::color(color::PURPLE), 1.),
    )
    .color_space(ColorSpace::Oklab)
}

/// The fill of anything that carries white text: purple to purple-deep, with an orange glint in
/// the top-right corner. White on the orange itself is only 2.3:1, so the label sits on purple.
/// `corner` is the element's radius; GPUI clips to rectangles, so the glint rounds itself.
fn cta_fill<E: gpui_kit::Styled + gpui_kit::ParentElement>(element: E, corner: Pixels) -> E {
    element
        .relative()
        .bg(linear_gradient(
            135.,
            linear_color_stop(theme::color(color::PURPLE), 0.),
            linear_color_stop(theme::color(color::PURPLE_DEEP), 1.),
        )
        .color_space(ColorSpace::Oklab))
        .shadow(theme::shadows(shadow::GLOSS))
        .child(
            div().absolute().top_0().left_0().size_full().rounded(corner).bg(linear_gradient(
                225.,
                linear_color_stop(theme::color(color::ORANGE).opacity(0.85), 0.),
                linear_color_stop(theme::color(color::ORANGE).opacity(0.), 0.55),
            )
            .color_space(ColorSpace::Oklab)),
        )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Plain,
    /// Where you are (the brand gradient).
    Current,
    /// A toggle that is switched off.
    Off,
    /// A small outlined circle: the actions in a pane header.
    Round,
}

/// The rounded square every icon sits in: `radius-md` up to 32px, `radius-lg` above; round tiles
/// are circles.
pub fn icon_tile(id: impl Into<ElementId>, name: &str, kind: TileKind, tile: f32) -> gpui_kit::Stateful<gpui_kit::Div> {
    let corner = match kind {
        TileKind::Round => tile / 2.,
        _ if tile > 32. => radius::RADIUS_LG,
        _ => radius::RADIUS_MD,
    };
    let quiet = matches!(kind, TileKind::Off | TileKind::Round);
    let glyph = theme::color(if quiet { color::INK_MUTED } else { color::INK });
    let tile_element = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(tile))
        .rounded(len(corner))
        .cursor_pointer()
        .map(|tile| match kind {
            TileKind::Plain => tile
                .bg(theme::color(color::CONTROL))
                .shadow(theme::shadows(shadow::LIFT))
                .hover(|s| s.bg(theme::color(color::CONTROL_HOVER))),
            TileKind::Current => tile.bg(brand_gradient()).shadow(theme::shadows(shadow::GLOSS)),
            TileKind::Off => tile.hover(|s| s.bg(theme::color(color::CONTROL))),
            TileKind::Round => tile
                .border_1()
                .border_color(theme::color(color::LINE_STRONG))
                .hover(|s| s.bg(theme::color(color::CONTROL))),
        });
    let glyph_size = match tile {
        t if t > 32. => 20.,
        t if t >= 32. => 16.,
        _ => 14.,
    };
    tile_element.child(icon(name, glyph_size, glyph))
}

/// A pill-shaped text button with an optional shortcut. `primary` is the one action a view is for.
pub fn button(
    id: impl Into<ElementId>,
    label: &'static str,
    shortcut: Option<&'static str>,
    primary: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let height = 32.;
    let element = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(height))
        .px(len(space::SPACE_5))
        .rounded_full()
        .cursor_pointer()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(color::INK));
    let element = if primary {
        cta_fill(element, px(height / 2.))
    } else {
        element
            .bg(theme::color(color::CONTROL))
            .shadow(theme::shadows(shadow::LIFT))
            .hover(|s| s.bg(theme::color(color::CONTROL_HOVER)))
    };
    let hint = if primary { theme::color(color::INK).opacity(0.72) } else { theme::color(color::INK_MUTED) };
    element.child(
        div()
            .relative()
            .flex()
            .items_center()
            .gap(len(space::SPACE_3))
            .child(label)
            .children(shortcut.map(|keys| div().font_weight(FontWeight::NORMAL).text_color(hint).child(keys))),
    )
}

/// Text whose `highlight` part runs through the brand gradient, one colour per character (GPUI has
/// no gradient text). One per screen.
pub fn gradient_text(text: &str, highlight: &str) -> StyledText {
    let full = format!("{text}{highlight}");
    let count = highlight.chars().count().max(2);
    #[expect(clippy::cast_precision_loss, reason = "a few characters")]
    let highlights = highlight
        .char_indices()
        .enumerate()
        .map(|(n, (at, c))| {
            let start = text.len() + at;
            let colour = theme::mix_oklab(color::ORANGE, color::PURPLE_TEXT, n as f32 / (count - 1) as f32);
            (start..start + c.len_utf8(), HighlightStyle { color: Some(colour), ..HighlightStyle::default() })
        })
        .collect::<Vec<_>>();
    StyledText::new(full).with_highlights(highlights)
}

/// The brand's halftone: `line-strong` dots at a 9px pitch, fading out downward.
pub fn dot_matrix() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
            const PITCH: f32 = 9.;
            let dot = theme::color(color::LINE_STRONG);
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let mut y = PITCH / 2.;
            while y < height {
                let fade = (1. - y / height).clamp(0., 1.);
                let mut x = PITCH / 2.;
                while x < width {
                    let origin = point(bounds.origin.x + px(x - 1.3), bounds.origin.y + px(y - 1.3));
                    window.paint_quad(
                        fill(Bounds::new(origin, size(px(2.6), px(2.6))), dot.opacity(fade)).corner_radii(px(1.3)),
                    );
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

/// The brand's diagonal lines: `line-strong` hairlines at 45°, fading away from the top-right
/// corner. For large empty areas.
pub fn diagonal_lines(ground: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        // GPUI clips to rectangles: round with the panel (its radius less its border).
        .rounded(len(radius::RADIUS_XL - 1.))
        .bg(pattern_slash(theme::color(color::LINE_STRONG), 1., 12.))
        .child(div().size_full().rounded(len(radius::RADIUS_XL - 1.)).bg(linear_gradient(
            225.,
            linear_color_stop(ground.opacity(0.), 0.),
            linear_color_stop(ground, 0.7),
        )))
}

/// A segmented switch whose selected pill is solid white.
pub fn pills(options: &[(&'static str, bool)]) -> impl IntoElement {
    div()
        .flex()
        .gap(px(2.))
        .p(px(3.))
        .rounded_full()
        .bg(theme::color(color::CONTROL))
        .shadow(theme::shadows(shadow::LIFT))
        .children(options.iter().map(|(label, selected)| {
            div()
                .px(len(space::SPACE_5))
                .h(px(28.))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .map(|pill| {
                    if *selected {
                        pill.bg(theme::color(color::INK)).text_color(theme::color(color::INK_INVERSE))
                    } else {
                        pill.text_color(theme::color(color::INK_MUTED))
                    }
                })
                .child(*label)
        }))
}

/// A panel on the ground: `surface`, `radius-xl`, one-pixel `line`.
pub fn panel() -> gpui_kit::Div {
    div()
        .size_full()
        .relative()
        .rounded(len(radius::RADIUS_XL))
        .bg(theme::color(color::SURFACE))
        .border_1()
        .border_color(theme::color(color::LINE))
        .overflow_hidden()
}

/// An uppercase label above a group: 11px, weight 600, in `ink-muted`.
pub fn eyebrow(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .line_height(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(color::INK_MUTED))
        .child(text.into().to_uppercase())
}

/// The sidebar until the file tree arrives (Phase 3): the project, the Files/Changes switch.
pub fn sidebar(project_name: &str, project_path: &str) -> AnyElement {
    panel()
        .flex()
        .flex_col()
        .gap(len(space::SPACE_5))
        .p(len(space::SPACE_2))
        .child(
            div()
                .flex()
                .items_center()
                .gap(len(space::SPACE_4))
                .child(icon_tile("project", "folder", TileKind::Current, 44.))
                .child(
                    div()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(15.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(project_name.to_owned()),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .line_height(px(14.))
                                .text_color(theme::color(color::INK_MUTED))
                                .truncate()
                                .child(project_path.to_owned()),
                        ),
                ),
        )
        .child(div().px(len(space::SPACE_1)).child(pills(&[("Files", true), ("Changes", false)])))
        .child(
            div().relative().flex_1().rounded(len(radius::RADIUS_LG)).overflow_hidden().child(dot_matrix()).child(
                div().flex().flex_col().gap(len(space::SPACE_2)).p(len(space::SPACE_4)).child(eyebrow("Files")).child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme::color(color::INK_MUTED))
                        .child("The file tree comes in Phase 3."),
                ),
            ),
        )
        .into_any_element()
}

/// The editor area until files open in it (Phase 3).
pub fn editor() -> AnyElement {
    panel()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(len(space::SPACE_3))
                .child(icon_tile("editor-file", "file", TileKind::Plain, 44.))
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No file open"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme::color(color::INK_MUTED))
                        .child("Opening and editing files comes in Phase 3. ⌘J hides this panel."),
                ),
        )
        .into_any_element()
}

/// What the grid shows with no terminal open: the diagonal lines, a light headline with one
/// gradient word, and the action.
pub fn empty_grid(
    project_name: &str,
    new_terminal: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    panel()
        .child(diagonal_lines(theme::color(color::SURFACE)))
        .child(
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
                            .child(eyebrow(project_name.to_owned()))
                            .child(
                                div()
                                    .text_size(px(28.))
                                    .line_height(px(32.))
                                    .font_weight(FontWeight::LIGHT)
                                    .child(gradient_text("No terminals ", "yet")),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme::color(color::INK_MUTED))
                                    .child("Open a shell in the project folder, then start Claude in it."),
                            ),
                    )
                    .child(button("empty-new", "New terminal", Some("⌘T"), true).on_click(new_terminal)),
            ),
        )
        .into_any_element()
}

/// How the draggable gaps between panels look: nothing at rest (the gap of ground is the
/// divider), a short purple pill while the pointer is on it or dragging.
pub fn quiet_resize_handle() -> gpui_kit::base::ResizeHandleRenderer {
    std::rc::Rc::new(|handle, _, _| {
        use gpui_kit::base::ResizeHandleState;
        // An empty element, not `None`: `None` brings back the library's own hairline.
        if handle.state() == ResizeHandleState::Idle {
            return Some(div().into_any_element());
        }
        let pill = theme::color(color::PURPLE_TEXT).opacity(if handle.state() == ResizeHandleState::Hovered {
            0.5
        } else {
            0.9
        });
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
