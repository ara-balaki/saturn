//! Comments on a review, for the agent: the + beside a line (or a selection of lines) opens a box
//! under it, comments gather under their lines as in a pull request review, and "Send to the agent"
//! hands them all over in one message, each with its file, lines and code (`saturn_core::review`).

use std::path::PathBuf;

use gpui_kit::component::diff::{DiffAnnotation, DiffLinePosition, DiffLineRange, DiffSide};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use saturn_core::review::{self, Comment};
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};

/// The comments are ready for the agent working in `repo`.
pub struct SendToAgent {
    pub repo: PathBuf,
    pub message: String,
}

struct Written {
    id: u64,
    range: DiffLineRange,
    text: String,
}

/// The comment being written, under the line it's about.
struct Writing {
    range: DiffLineRange,
    input: Entity<InputState>,
    _events: Subscription,
}

pub struct ReviewComments {
    /// Where the reviewed files are.
    repo: PathBuf,
    written: Vec<Written>,
    writing: Option<Writing>,
    next: u64,
}

impl EventEmitter<SendToAgent> for ReviewComments {}

impl ReviewComments {
    pub fn new(repo: PathBuf) -> Self {
        Self { repo, written: Vec::new(), writing: None, next: 0 }
    }

    pub fn repo(&self) -> &std::path::Path {
        &self.repo
    }

    /// Opens a box for a comment on `range`.
    pub fn start(&mut self, range: DiffLineRange, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("What should change here?"));
        input.update(cx, |input, cx| input.focus(window, cx));
        let events = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.add(cx);
            }
        });
        self.writing = Some(Writing { range, input, _events: events });
        cx.notify();
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let Some(writing) = self.writing.take() else { return };
        let text = writing.input.read(cx).value().trim().to_owned();
        if !text.is_empty() {
            self.next += 1;
            self.written.push(Written { id: self.next, range: writing.range, text });
        }
        cx.notify();
    }

    /// Adds a comment without the box: for spikes (`SATURN_REVIEW_COMMENT`).
    pub fn comment(&mut self, range: DiffLineRange, text: &str, cx: &mut Context<Self>) {
        self.next += 1;
        self.written.push(Written { id: self.next, range, text: text.to_owned() });
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.writing = None;
        cx.notify();
    }

    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        self.written.retain(|written| written.id != id);
        cx.notify();
    }

    pub fn discard(&mut self, cx: &mut Context<Self>) {
        self.written.clear();
        self.writing = None;
        cx.notify();
    }

    /// Hands every comment to the agent, with the code each is about, and clears them.
    pub fn send(&mut self, cx: &mut Context<Self>) {
        // A comment still being written goes too.
        self.add(cx);
        if self.written.is_empty() {
            return;
        }
        let comments: Vec<Comment> = self
            .written
            .drain(..)
            .map(|written| {
                let path = written.range.path().to_string();
                let lines =
                    (written.range.start().min(written.range.end()), written.range.start().max(written.range.end()));
                let removed = written.range.side() == DiffSide::Original;
                // What's there now is on disk; removed lines are only in the diff, so they go unquoted.
                let code = if removed {
                    Vec::new()
                } else {
                    std::fs::read_to_string(self.repo.join(&path))
                        .map(|text| review::lines_of(&text, lines))
                        .unwrap_or_default()
                };
                Comment { path, lines, removed, code, text: written.text }
            })
            .collect();
        cx.emit(SendToAgent { repo: self.repo.clone(), message: review::message(&comments) });
        cx.notify();
    }

    /// Where each comment shows: under the last line it's about.
    pub fn annotations(&self) -> Vec<DiffAnnotation> {
        let under = |range: &DiffLineRange| DiffLinePosition::new(range.path().clone(), range.end_side(), range.end());
        let mut annotations: Vec<DiffAnnotation> = self
            .written
            .iter()
            .map(|written| DiffAnnotation::line(("review-comment", written.id), under(&written.range)))
            .collect();
        if let Some(writing) = &self.writing {
            annotations.push(DiffAnnotation::line("review-writing", under(&writing.range)));
        }
        annotations
    }

    /// A comment, or the box for the one being written.
    pub fn render_annotation(&self, annotation: &DiffAnnotation, cx: &mut Context<Self>) -> AnyElement {
        let card = || {
            div()
                .flex()
                .items_start()
                .gap(len(space::SPACE_2))
                .my(len(space::SPACE_1))
                .mx(len(space::SPACE_3))
                .p(len(space::SPACE_2))
                .rounded(len(radius::RADIUS_SM))
                .bg(theme::color(color::SURFACE))
                .border_1()
                .border_color(theme::separator())
                .text_size(px(13.))
        };
        if annotation.id() == &gpui_kit::ElementId::from("review-writing") {
            let Some(writing) = &self.writing else { return div().into_any_element() };
            return card()
                .items_center()
                .child(div().flex_1().min_w_0().child(Input::new(&writing.input)))
                .child(
                    chrome::button("review-add", "Add comment", None, true)
                        .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
                )
                .child(
                    chrome::button("review-cancel", "Cancel", None, false)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                )
                .into_any_element();
        }
        let Some(written) = self
            .written
            .iter()
            .find(|written| annotation.id() == &gpui_kit::ElementId::from(("review-comment", written.id)))
        else {
            return div().into_any_element();
        };
        let id = written.id;
        card()
            .child(div().flex_1().min_w_0().text_color(theme::color(color::INK)).child(written.text.clone()))
            .child(
                chrome::icon_button(("review-remove", id), "x", true)
                    .tooltip(chrome::tip("Remove this comment"))
                    .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx))),
            )
            .into_any_element()
    }

    /// Above the review while there are comments: how many, and what to do with them.
    pub fn render_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let count = self.written.len();
        if count == 0 {
            return None;
        }
        let what =
            if count == 1 { "1 comment for the agent".to_owned() } else { format!("{count} comments for the agent") };
        Some(
            div()
                .id("review-comments-bar")
                .flex()
                .flex_none()
                .items_center()
                .gap(len(space::SPACE_2))
                .px(len(space::SPACE_3))
                .py(len(space::SPACE_1))
                .border_b_1()
                .border_color(theme::separator())
                .text_size(px(13.))
                .child(div().flex_1().text_color(theme::color(color::INK_MUTED)).child(what))
                .child(
                    chrome::button("review-discard", "Discard", None, false)
                        .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
                )
                .child(
                    chrome::button("review-send", "Send to the agent", None, true)
                        .on_click(cx.listener(|this, _, _, cx| this.send(cx))),
                )
                .into_any_element(),
        )
    }
}
