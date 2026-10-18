//! The Settings dialog (⌘,).

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::{App, FontWeight, IntoElement, ParentElement as _, Styled as _, WeakEntity, Window, div, px};
use saturn_design::{color, space};

use crate::settings::Settings;
use crate::theme::{self, len};
use crate::workspace::Workspace;

fn setting_row(
    id: &'static str,
    title: &'static str,
    detail: &'static str,
    checked: bool,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap(len(space::SPACE_6))
        .py(len(space::SPACE_3))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(len(space::SPACE_1))
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(title))
                .child(div().text_size(px(12.)).text_color(theme::color(color::INK_MUTED)).child(detail)),
        )
        .child(div().flex_none().pt(len(space::SPACE_1)).child(Switch::new(id).checked(checked).on_click(on_change)))
}

/// Opens the Settings dialog. Each switch saves at once.
pub fn open(workspace: WeakEntity<Workspace>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let settings = workspace.upgrade().map(|w| w.read(cx).settings().clone()).unwrap_or_default();
        let change = |workspace: WeakEntity<Workspace>, apply: fn(&mut Settings, bool)| {
            move |checked: &bool, _: &mut Window, cx: &mut App| {
                let checked = *checked;
                let _ = workspace.update(cx, |workspace, cx| workspace.change_settings(|settings| apply(settings, checked), cx));
            }
        };
        dialog
            .title("Settings")
            .w(px(480.))
            .child(setting_row(
                "name-tabs",
                "Name tabs after Claude's session",
                "A tab shows the session's name, from /rename or set automatically. Off: the tab shows its folder, or a name you gave it by double-clicking.",
                settings.name_tabs_after_sessions,
                change(workspace.clone(), |settings, on| settings.name_tabs_after_sessions = on),
            ))
            .child(setting_row(
                "rename-sessions",
                "Name sessions after tickets",
                "A slash command whose first word is a ticket, like /start abc-224, renames the session to ABC-224, as /rename would. A name you gave yourself is never replaced.",
                settings.rename_sessions_from_commands,
                change(workspace.clone(), |settings, on| settings.rename_sessions_from_commands = on),
            ))
    });
}
