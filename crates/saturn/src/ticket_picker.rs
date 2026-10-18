//! The ticket picker (⌘⇧T): your open ClickUp tickets, searchable and grouped by status. Picking
//! one starts an agent on it in a new pane, the way the project's ticket settings say.

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    App, AppContext as _, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px, rgb,
};
use saturn_core::clickup::{self, Ticket};
use saturn_design::{color, radius, space};

use crate::theme::{self, len};
use crate::workspace::Workspace;

pub fn open(workspace: WeakEntity<Workspace>, window: &mut Window, cx: &mut App) {
    let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tickets by id, title or status"));
    search.update(cx, |search, cx| search.focus(window, cx));
    if let Some(workspace) = workspace.upgrade() {
        workspace.update(cx, Workspace::refresh_tickets);
    }
    window.open_dialog(cx, move |dialog, _, cx| {
        let Some(workspace) = workspace.upgrade() else { return dialog };
        let state = workspace.read(cx).clickup();
        let query = search.read(cx).value().to_string();
        let tickets: Vec<Ticket> = state.tickets.iter().filter(|ticket| ticket.matches(&query)).cloned().collect();
        let note = if !state.connected() {
            Some("Connect ClickUp in Settings (⌘,) to start agents from your tickets.".to_owned())
        } else if let Some(problem) = &state.problem {
            Some(problem.clone())
        } else if state.loading {
            Some("Reading your tickets…".to_owned())
        } else if tickets.is_empty() {
            Some(if query.is_empty() { "No open tickets assigned to you." } else { "No ticket matches." }.to_owned())
        } else {
            None
        };
        dialog
            .title("New agent")
            .w(px(600.))
            .child(Input::new(&search))
            .children(note.map(|note| {
                div().py(len(space::SPACE_4)).text_size(px(14.)).text_color(theme::color(color::INK_MUTED)).child(note)
            }))
            .child(list(&workspace, &tickets))
    });
}

fn list(workspace: &Entity<Workspace>, tickets: &[Ticket]) -> impl IntoElement {
    let mut rows = div().id("tickets").flex().flex_col().max_h(px(440.)).overflow_y_scroll().mt(len(space::SPACE_2));
    for (status, group) in clickup::by_status(tickets) {
        rows = rows.child(
            div()
                .pt(len(space::SPACE_3))
                .pb(len(space::SPACE_1))
                .px(len(space::SPACE_3))
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::color(color::INK_FAINT))
                .child(status.to_uppercase()),
        );
        for ticket in group {
            rows = rows.child(row(workspace, ticket));
        }
    }
    rows
}

fn row(workspace: &Entity<Workspace>, ticket: &Ticket) -> impl IntoElement {
    let workspace = workspace.downgrade();
    let picked = ticket.clone();
    div()
        .id(gpui_kit::SharedString::from(format!("ticket-{}", ticket.id)))
        .flex()
        .items_center()
        .gap(len(space::SPACE_3))
        .px(len(space::SPACE_3))
        .py(len(space::SPACE_2))
        .rounded(len(radius::RADIUS_SM))
        .cursor_pointer()
        .hover(|s| s.bg(theme::color(color::CONTROL)))
        .child(status_dot(ticket))
        .child(
            div()
                .flex_none()
                .w(px(84.))
                .font_family(saturn_design::font::MONO)
                .text_size(px(12.))
                .text_color(theme::color(color::INK_MUTED))
                .truncate()
                .child(ticket.key().to_owned()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(14.))
                .text_color(theme::color(color::INK))
                .truncate()
                .child(ticket.title.clone()),
        )
        .children(
            ticket.list.clone().map(|list| {
                div().flex_none().text_size(px(12.)).text_color(theme::color(color::INK_FAINT)).child(list)
            }),
        )
        .on_click(move |_, window, cx| {
            if let Some(workspace) = workspace.upgrade() {
                workspace.update(cx, |workspace, cx| workspace.start_ticket(&picked, window, cx));
            }
            window.close_dialog(cx);
        })
}

/// A dot in the status's own ClickUp colour, so statuses read as they do in ClickUp.
pub fn status_dot(ticket: &Ticket) -> impl IntoElement {
    let colour = ticket
        .status_color
        .as_deref()
        .and_then(|hex| u32::from_str_radix(hex.trim_start_matches('#'), 16).ok())
        .map_or_else(|| theme::color(color::INK_FAINT), |value| rgb(value).into());
    div().flex_none().size(px(8.)).rounded_full().bg(colour)
}
