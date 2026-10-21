//! Menus that open from a button: the New agent arrow, a pane's ⋯ and the repo's ⋯. They hold what
//! would otherwise be found only by typing in the command palette, grouped by where it applies.
//! Each item dispatches the same action the palette and the menu bar do, so all three stay in step.

use gpui_kit::{
    Action, Anchor, AnyElement, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, anchored, deferred, div, px,
};
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};

/// One line of a menu: an icon, what it does, and the action that does it. `None` is a separator.
pub type Item = Option<(&'static str, &'static str, Box<dyn Action>)>;

/// The menu's lines, under the button that opened it, its right edge on the button's. `pick` gets
/// the chosen action; `dismiss` is called for a click outside.
pub fn render(
    id: &'static str,
    items: Vec<Item>,
    pick: impl Fn(Box<dyn Action>, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
    dismiss: impl Fn(&mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    let pick = std::rc::Rc::new(pick);
    let mut list = chrome::floating(div())
        .id(id)
        .flex()
        .flex_col()
        .min_w(px(240.))
        .p(len(space::SPACE_1))
        .rounded(len(radius::RADIUS_MD))
        .occlude()
        .on_mouse_down_out(move |_, window, cx| dismiss(window, cx));
    for (index, item) in items.into_iter().enumerate() {
        let Some((icon, label, action)) = item else {
            list = list.child(div().h(px(1.)).mx(len(space::SPACE_2)).my(len(space::SPACE_1)).bg(theme::separator()));
            continue;
        };
        let pick = pick.clone();
        list = list.child(
            div()
                .id(("menu-item", index))
                .flex()
                .items_center()
                .gap(len(space::SPACE_3))
                .h(len(chrome::CONTROL_HEIGHT))
                .px(len(space::SPACE_2))
                .rounded(len(radius::RADIUS_SM))
                .cursor_pointer()
                .hover(|s| s.bg(theme::fill_hover()))
                .text_size(px(13.))
                .font_weight(FontWeight::NORMAL)
                .text_color(theme::color(color::INK))
                .child(chrome::icon(icon, 14., theme::color(color::INK_MUTED)))
                .child(label)
                .on_click(move |_, window, cx| pick(action.boxed_clone(), window, cx)),
        );
    }
    // A point at the button's bottom right, from which the menu hangs, drawn above everything.
    div()
        .absolute()
        .top_full()
        .right_0()
        .child(
            deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(list.mt(px(4.))))
                .with_priority(1),
        )
        .into_any_element()
}
