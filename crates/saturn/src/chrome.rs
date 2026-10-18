//! The window's chrome in the design system: icon tiles, pills, the dot matrix, and the sidebar,
//! editor and empty-grid placeholders.

use gpui_kit::{
    AnyElement, App, Bounds, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, fill, linear_color_stop,
    linear_gradient, point, prelude::FluentBuilder as _, px, size, svg,
};
use saturn_design::{color, radius, space};

use crate::theme::{self, len};

/// A Lucide icon from gpui-kit's bundled set, e.g. `panel-left`. SVGs don't inherit the text
/// colour, so it is given here.
pub fn icon(name: &str, size: f32, color: gpui_kit::Hsla) -> impl IntoElement {
    svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).flex_none().text_color(color)
}

/// The brand gradient (orange → purple), for the tile that marks where you are.
pub fn brand_gradient() -> gpui_kit::Background {
    linear_gradient(
        135.,
        linear_color_stop(theme::color(color::ORANGE), 0.),
        linear_color_stop(theme::color(color::PURPLE), 1.),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Plain,
    /// The primary action of a view (`purple`).
    Accent,
    /// Where you are (the brand gradient).
    Current,
    /// A toggle that is switched off.
    Off,
}

/// The rounded square every icon sits in. 26px tiles sit 4px inside 14px panels (14 − 4 = 10).
pub fn icon_tile(id: impl Into<ElementId>, name: &str, kind: TileKind, tile: f32) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(tile))
        .rounded(len(radius::RADIUS_MD))
        .text_color(theme::color(if kind == TileKind::Off { color::INK_MUTED } else { color::INK }))
        .map(|tile| match kind {
            TileKind::Plain => {
                tile.bg(theme::color(color::CONTROL)).hover(|s| s.bg(theme::color(color::CONTROL_HOVER)))
            }
            TileKind::Accent => tile.bg(theme::color(color::PURPLE)).hover(|s| s.bg(theme::color(color::PURPLE_DEEP))),
            TileKind::Current => tile.bg(brand_gradient()),
            TileKind::Off => tile.hover(|s| s.bg(theme::color(color::CONTROL))),
        })
        .cursor_pointer()
        .child(icon(
            name,
            if tile >= 32. { 16. } else { 14. },
            theme::color(if kind == TileKind::Off { color::INK_MUTED } else { color::INK }),
        ))
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

/// A segmented switch whose selected pill is solid white.
pub fn pills(options: &[(&'static str, bool)]) -> impl IntoElement {
    div().flex().gap(len(space::SPACE_1)).p(px(3.)).rounded_full().bg(theme::color(color::SURFACE_RAISED)).children(
        options.iter().map(|(label, selected)| {
            div()
                .px(len(space::SPACE_4))
                .h(px(26.))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .map(|pill| {
                    if *selected {
                        pill.bg(theme::color(color::INK)).text_color(theme::color(color::INK_INVERSE))
                    } else {
                        pill.text_color(theme::color(color::INK_MUTED))
                    }
                })
                .child(*label)
        }),
    )
}

/// A panel on the ground: `surface`, `radius-lg`, one-pixel `line`.
pub fn panel() -> gpui_kit::Div {
    div()
        .size_full()
        .relative()
        .rounded(len(radius::RADIUS_LG))
        .bg(theme::color(color::SURFACE))
        .border_1()
        .border_color(theme::color(color::LINE))
        .overflow_hidden()
}

/// The sidebar until the file tree arrives (Phase 3): the project, the Files/Changes switch.
pub fn sidebar(project_name: &str, project_path: &str) -> AnyElement {
    panel()
        .flex()
        .flex_col()
        .gap(len(space::SPACE_4))
        .p(len(space::SPACE_1))
        .child(
            div()
                .flex()
                .items_center()
                .gap(len(space::SPACE_4))
                .child(icon_tile("project", "folder", TileKind::Current, 32.))
                .child(
                    div()
                        .min_w_0()
                        .child(div().text_size(px(15.)).font_weight(FontWeight::MEDIUM).child(project_name.to_owned()))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme::color(color::INK_MUTED))
                                .truncate()
                                .child(project_path.to_owned()),
                        ),
                ),
        )
        .child(div().px(len(space::SPACE_1)).child(pills(&[("Files", true), ("Changes", false)])))
        .child(
            div().relative().flex_1().rounded(len(radius::RADIUS_MD)).overflow_hidden().child(dot_matrix()).child(
                div()
                    .p(len(space::SPACE_4))
                    .text_size(px(13.))
                    .text_color(theme::color(color::INK_MUTED))
                    .child("The file tree comes in Phase 3."),
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
                .child(icon_tile("editor-file", "file", TileKind::Plain, 32.))
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("No file open"))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme::color(color::INK_MUTED))
                        .child("Opening and editing files comes in Phase 3. ⌘J hides this panel."),
                ),
        )
        .into_any_element()
}

/// What the grid shows with no terminal open.
pub fn empty_grid(new_terminal: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static) -> AnyElement {
    panel()
        .child(dot_matrix())
        .child(
            div().absolute().size_full().flex().items_center().justify_center().child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(len(space::SPACE_3))
                    .child(icon_tile("empty-new", "plus", TileKind::Accent, 32.).on_click(new_terminal))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("New terminal"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme::color(color::INK_MUTED))
                            .child("⌘T opens a shell in the project folder"),
                    ),
            ),
        )
        .into_any_element()
}
