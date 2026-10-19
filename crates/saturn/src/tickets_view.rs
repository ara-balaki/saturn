//! The Tickets panel, on the window's right (⌘⌥B): the ClickUp tickets assigned to you, by status,
//! searchable. Clicking one starts an agent on it, as the ticket picker does; one that already has
//! a pane says so. Without ClickUp it says how to connect it.

use std::collections::HashSet;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};
use saturn_core::clickup::{self, Ticket};
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};
use crate::ticket_picker::status_dot;
use crate::workspace::{OpenSettings, Workspace};

pub struct TicketsView {
    workspace: WeakEntity<Workspace>,
    search: Entity<InputState>,
    _search_events: Subscription,
    /// Redraws when the workspace's tickets change.
    _workspace: Option<Subscription>,
}

impl TicketsView {
    pub fn new(workspace: WeakEntity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tickets"));
        let search_events = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let observed = workspace.upgrade().map(|workspace| cx.observe(&workspace, |_, _, cx| cx.notify()));
        Self { workspace, search, _search_events: search_events, _workspace: observed }
    }

    fn render_ticket(ticket: &Ticket, running: bool, cx: &mut Context<Self>) -> AnyElement {
        let (picked, url) = (ticket.clone(), ticket.url.clone());
        let group = gpui_kit::SharedString::from(format!("ticket-{}", ticket.id));
        div()
            .id(gpui_kit::SharedString::from(format!("panel-ticket-{}", ticket.id)))
            .group(group.clone())
            .flex()
            .items_start()
            .gap(len(space::SPACE_2))
            .px(len(space::SPACE_2))
            .py(len(space::SPACE_2))
            .rounded(len(radius::RADIUS_SM))
            .cursor_pointer()
            .hover(|s| s.bg(theme::fill_hover()))
            .child(div().pt(px(5.)).child(status_dot(ticket)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(len(space::SPACE_2))
                            .text_size(px(11.))
                            .font_family(saturn_design::font::MONO)
                            .text_color(theme::color(color::INK_MUTED))
                            .child(ticket.key().to_owned())
                            .when(running, |line| {
                                line.child(
                                    div()
                                        .font_family(saturn_design::font::SANS)
                                        .text_color(theme::color(color::BLUE))
                                        .child("● agent running"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(theme::color(color::INK))
                            .child(ticket.title.clone()),
                    ),
            )
            .child(
                chrome::icon_button(gpui_kit::SharedString::from(format!("open-{}", ticket.id)), "external-link", true)
                    .invisible()
                    .group_hover(group, gpui_kit::Styled::visible)
                    .tooltip(chrome::tip("Open in ClickUp"))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = std::process::Command::new("open").arg(&url).spawn();
                    }),
            )
            .tooltip(chrome::tip("Start an agent on this ticket"))
            .on_click(cx.listener(move |this, _, window, cx| {
                let _ = this.workspace.update(cx, |workspace, cx| workspace.start_ticket(&picked, window, cx));
            }))
            .into_any_element()
    }
}

impl Render for TicketsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(workspace) = self.workspace.upgrade() else { return div().into_any_element() };
        let state = workspace.read(cx).clickup();
        let query = self.search.read(cx).value().to_string();
        let tickets: Vec<Ticket> = state.tickets.iter().filter(|ticket| ticket.matches(&query)).cloned().collect();
        let running: HashSet<String> = workspace.read(cx).tickets_with_agents(cx);
        let note = if !state.connected() {
            Some("Connect ClickUp in Settings to see the tickets assigned to you here.")
        } else if state.loading {
            Some("Reading your tickets…")
        } else if tickets.is_empty() {
            Some(if query.trim().is_empty() { "No open tickets assigned to you." } else { "No ticket matches." })
        } else {
            None
        };
        let problem = state.problem.clone();
        let connected = state.connected();
        let groups = clickup::by_status(&tickets);
        let mut list = div().id("tickets-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col();
        for (status, group) in groups {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(len(space::SPACE_2))
                    .pt(len(space::SPACE_3))
                    .pb(len(space::SPACE_1))
                    .px(len(space::SPACE_2))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::color(color::INK_FAINT))
                    .child(status.to_uppercase())
                    .child(group.len().to_string()),
            );
            for ticket in group {
                list = list.child(Self::render_ticket(ticket, running.contains(ticket.key()), cx));
            }
        }
        chrome::panel()
            .pb(len(space::SPACE_1))
            .gap(len(space::SPACE_1))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(len(space::SPACE_2))
                    .h(len(chrome::CONTROL_HEIGHT))
                    .pl(len(space::SPACE_2))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Tickets"))
                    .when(connected, |head| {
                        head.child(
                            chrome::icon_button("tickets-refresh", "refresh-cw", true)
                                .tooltip(chrome::tip("Read the tickets again"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let _ = this.workspace.update(cx, Workspace::refresh_tickets);
                                })),
                        )
                    }),
            )
            .when(connected, |panel| {
                panel.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(len(space::SPACE_2))
                        .h(len(chrome::CONTROL_HEIGHT))
                        .px(len(space::SPACE_2))
                        .rounded(len(radius::RADIUS_SM))
                        .bg(theme::fill_control())
                        .child(chrome::icon("search", 13., theme::color(color::INK_FAINT)))
                        .child(div().flex_1().child(Input::new(&self.search).appearance(false))),
                )
            })
            .children(problem.map(|problem| {
                div().px(len(space::SPACE_2)).text_size(px(12.)).text_color(theme::color(color::RED)).child(problem)
            }))
            .children(note.map(|note| {
                div()
                    .flex()
                    .flex_col()
                    .gap(len(space::SPACE_3))
                    .p(len(space::SPACE_2))
                    .text_size(px(13.))
                    .text_color(theme::color(color::INK_MUTED))
                    .child(note)
                    .when(!connected, |note| {
                        note.child(
                            chrome::button("tickets-connect", "Open Settings", Some("⌘,"), false)
                                .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx)),
                        )
                    })
            }))
            .child(list)
            .into_any_element()
    }
}
