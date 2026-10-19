//! The Settings dialog (⌘,).

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::{
    App, AppContext as _, Entity, FontWeight, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, Window, div, px,
};
use saturn_design::{color, space};

use crate::chrome;
use crate::settings::{AppearanceSetting, Settings};
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
                .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(title))
                .child(div().text_size(px(12.)).text_color(theme::color(color::INK_MUTED)).child(detail)),
        )
        .child(div().flex_none().pt(len(space::SPACE_1)).child(Switch::new(id).checked(checked).on_click(on_change)))
}

/// System / Light / Dark as a segmented track; the selected one is raised.
fn appearance_row(workspace: &WeakEntity<Workspace>, current: AppearanceSetting) -> impl IntoElement {
    let options =
        [("System", AppearanceSetting::System), ("Light", AppearanceSetting::Light), ("Dark", AppearanceSetting::Dark)];
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(len(space::SPACE_6))
        .py(len(space::SPACE_3))
        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Appearance"))
        .child(chrome::track().children(options.map(|(label, setting)| {
            let workspace = workspace.clone();
            chrome::track_item(label, setting == current).px(len(space::SPACE_4)).child(label).on_click(
                move |_, _, cx| {
                    let _ = workspace.update(cx, |workspace, cx| {
                        workspace.change_settings(|settings| settings.appearance = setting, cx);
                    });
                },
            )
        })))
}

/// The ClickUp section: a token to connect with, or the connection and this project's ticket
/// settings. Optional: without a token, Saturn shows nothing about ClickUp anywhere else.
fn clickup_section(
    workspace: &WeakEntity<Workspace>,
    token: &Entity<InputState>,
    command: &Entity<InputState>,
    cx: &App,
) -> impl IntoElement {
    let Some(workspace_entity) = workspace.upgrade() else { return div() };
    let workspace_view = workspace_entity.read(cx);
    let state = workspace_view.clickup();
    let project = workspace_view.project_path().to_owned();
    let project_settings = workspace_view.settings().project(&project);
    let project_name =
        project.file_name().map_or_else(|| project.display().to_string(), |n| n.to_string_lossy().into_owned());
    let muted = |text: String| div().text_size(px(12.)).text_color(theme::color(color::INK_MUTED)).child(text);
    let mut section = div()
        .flex()
        .flex_col()
        .gap(len(space::SPACE_3))
        .pt(len(space::SPACE_5))
        .mt(len(space::SPACE_3))
        .border_t_1()
        .border_color(theme::separator())
        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("ClickUp"));
    if state.connected() {
        let disconnect = {
            let workspace = workspace.clone();
            chrome::button("clickup-disconnect", "Disconnect", None, false).on_click(move |_, _, cx| {
                let _ = workspace.update(cx, Workspace::disconnect_clickup);
            })
        };
        let account =
            state.account.clone().map_or_else(|| "Connected.".to_owned(), |name| format!("Connected to {name}."));
        section = section
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(len(space::SPACE_4))
                    .child(muted(format!("{account} ⌘⇧T starts an agent on one of your tickets.")))
                    .child(disconnect),
            )
            .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(format!("Starting a ticket in {project_name}")))
            .child(Input::new(command))
            .child(muted("Runs in the new pane. {ticket} becomes the ticket's id (ABC-236), {title} its title.".to_owned()))
            .child(setting_row(
                "ticket-worktree",
                "A worktree per ticket",
                "Saturn makes a git worktree next to the project, on a branch named after the ticket, and opens the pane there. Off when the command sets up its own.",
                project_settings.create_worktree,
                {
                    let workspace = workspace.clone();
                    let project = project.clone();
                    move |on: &bool, _: &mut Window, cx: &mut App| {
                        let (on, project) = (*on, project.clone());
                        let _ = workspace.update(cx, |workspace, cx| {
                            workspace.change_settings(|settings| settings.projects.entry(project).or_default().create_worktree = on, cx);
                        });
                    }
                },
            ));
    } else {
        let connect = {
            let (workspace, token) = (workspace.clone(), token.clone());
            chrome::button("clickup-connect", "Connect", None, true).on_click(move |_, _, cx| {
                let value = token.read(cx).value().trim().to_owned();
                if !value.is_empty() {
                    let _ = workspace.update(cx, |workspace, cx| workspace.connect_clickup(value, cx));
                }
            })
        };
        section = section
            .child(muted(
                "Start agents from your tickets and see their status on each pane. Paste a personal API token (ClickUp → Settings → Apps); Saturn keeps it in the macOS Keychain.".to_owned(),
            ))
            .child(div().flex().items_center().gap(len(space::SPACE_2)).child(div().flex_1().child(Input::new(token))).child(connect));
    }
    if state.loading {
        section = section.child(muted("Checking with ClickUp…".to_owned()));
    }
    if let Some(problem) = &state.problem {
        section = section.child(div().text_size(px(12.)).text_color(theme::color(color::RED)).child(problem.clone()));
    }
    section
}

/// Opens the Settings dialog. `ticket_command` is the project's current one: the caller passes it,
/// since the workspace that opens the dialog is in the middle of an update and can't be read.
pub fn open(workspace: WeakEntity<Workspace>, ticket_command: String, window: &mut Window, cx: &mut App) {
    let token = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("pk_…"));
    let command = cx.new(|cx| InputState::new(window, cx).default_value(ticket_command));
    // The command saves as it is typed.
    cx.subscribe(&command, {
        let workspace = workspace.clone();
        move |input, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let value = input.read(cx).value().to_string();
            let _ = workspace.update(cx, |workspace, cx| {
                let project = workspace.project_path().to_owned();
                workspace.change_settings(
                    |settings| settings.projects.entry(project).or_default().ticket_command = value,
                    cx,
                );
            });
        }
    })
    .detach();
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
            .entrance(gpui_kit::component::dialog::DialogEntrance::None)
            .bg(chrome::dialog_surface())
            .w(px(480.))
            .child(appearance_row(&workspace, settings.appearance))
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
            .child(setting_row(
                "format-on-save",
                "Format on save",
                "Saving a file runs the project's own formatter on it (oxfmt, Prettier, Biome or rustfmt), only when the project has a config for one and has it installed.",
                settings.format_on_save,
                change(workspace.clone(), |settings, on| settings.format_on_save = on),
            ))
            .child(setting_row(
                "notifications",
                "Notifications",
                "When an agent needs you or finishes while Saturn is in the background, macOS shows a notification. Only in Saturn.app.",
                settings.notifications,
                change(workspace.clone(), |settings, on| settings.notifications = on),
            ))
            .child(setting_row(
                "menu-bar",
                "Menu bar icon",
                "Saturn in the menu bar, with how many agents are waiting for you, so you see it while Saturn is hidden. Clicking it takes you to the next one.",
                settings.menu_bar,
                change(workspace.clone(), |settings, on| settings.menu_bar = on),
            ))
            .child(clickup_section(&workspace, &token, &command, cx))
    });
}
