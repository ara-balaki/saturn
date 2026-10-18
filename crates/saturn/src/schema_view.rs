//! The database diagram, in the editor panel: every table as a box with its columns, and a line for
//! every foreign key. Scroll or drag to pan, pinch (or ⌘-scroll) to zoom. Clicking a table focuses
//! it: its relations light up and unrelated tables step back.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _, PathBuilder, PinchEvent, Pixels, Point, Render,
    ScrollWheelEvent, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, point,
    prelude::FluentBuilder as _, px,
};
use saturn_core::schema::{self, HEADER_HEIGHT, Key, Placed, ROW_HEIGHT, Schema};
use saturn_design::{color, radius, space};

use crate::chrome;
use crate::theme::{self, len};

const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 2.;
/// Space around the diagram when it's fitted to the view.
const MARGIN: f32 = 32.;

pub struct SchemaView {
    repo: PathBuf,
    schema: Schema,
    placed: Vec<Placed>,
    loaded: bool,
    zoom: f32,
    /// Where the diagram's origin sits in the view, in pixels.
    offset: Point<Pixels>,
    /// The view's bounds on screen, from the last frame: events come in window coordinates.
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Fit the diagram to the view once its size is known.
    fit_pending: bool,
    /// Where a drag started, to pan by.
    dragging_from: Option<Point<Pixels>>,
    focused: Option<usize>,
}

impl SchemaView {
    pub fn new(repo: &Path, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            repo: repo.to_owned(),
            schema: Schema::default(),
            placed: Vec::new(),
            loaded: false,
            zoom: 1.,
            offset: point(px(MARGIN), px(MARGIN)),
            bounds: Rc::new(Cell::new(None)),
            fit_pending: true,
            dragging_from: None,
            focused: None,
        };
        if std::env::var_os("SATURN_SCHEMA").is_some() {
            // Screenshots: the test window stops drawing before a background read would land.
            view.schema = schema::load(repo);
            view.placed = schema::layout(&view.schema);
            view.loaded = true;
        } else {
            view.reload(cx);
        }
        view
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// Reads the schema files again, off the main thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let (schema, placed) = cx
                .background_spawn(async move {
                    let schema = schema::load(&repo);
                    let placed = schema::layout(&schema);
                    (schema, placed)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.schema = schema;
                this.placed = placed;
                this.loaded = true;
                this.fit_pending = true;
                cx.notify();
            });
        })
        .detach();
    }

    /// The whole diagram in diagram units.
    fn extent(&self) -> (f32, f32) {
        self.placed.iter().fold((0., 0.), |(w, h), p| (w.max(p.x + p.width), h.max(p.y + p.height)))
    }

    /// Zooms so the whole diagram fits, at most 100%.
    fn fit(&mut self) {
        let Some(bounds) = self.bounds.get() else { return };
        let (width, height) = self.extent();
        let (zoom, x, y) =
            schema::fit(width, height, f32::from(bounds.size.width), f32::from(bounds.size.height), MARGIN, MIN_ZOOM);
        self.zoom = zoom;
        self.offset = point(px(x), px(y));
        self.fit_pending = false;
    }

    /// Zooms by `factor` keeping the diagram point under `anchor` (window coordinates) in place.
    fn zoom_at(&mut self, factor: f32, anchor: Point<Pixels>, cx: &mut Context<Self>) {
        let origin = self.bounds.get().map_or(point(px(0.), px(0.)), |bounds| bounds.origin);
        let local = anchor - origin;
        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let scale = new_zoom / self.zoom;
        self.offset = point(local.x - (local.x - self.offset.x) * scale, local.y - (local.y - self.offset.y) * scale);
        self.zoom = new_zoom;
        cx.notify();
    }

    fn zoom_centre(&mut self, factor: f32, cx: &mut Context<Self>) {
        let centre = self.bounds.get().map_or(point(px(0.), px(0.)), |bounds| bounds.center());
        self.zoom_at(factor, centre, cx);
    }

    /// Tables related to the focused one, and itself.
    fn related(&self) -> Option<Vec<bool>> {
        let focused = self.focused?;
        let name = &self.schema.tables[focused].name;
        let mut related = vec![false; self.schema.tables.len()];
        related[focused] = true;
        for relation in &self.schema.relations {
            let other = if &relation.from_table == name {
                &relation.to_table
            } else if &relation.to_table == name {
                &relation.from_table
            } else {
                continue;
            };
            if let Some(index) = self.schema.tables.iter().position(|table| &table.name == other) {
                related[index] = true;
            }
        }
        Some(related)
    }

    fn render_table(&self, index: usize, dimmed: bool, cx: &mut Context<Self>) -> AnyElement {
        let table = &self.schema.tables[index];
        let place = self.placed[index];
        let z = self.zoom;
        let focused = self.focused == Some(index);
        div()
            .id(("table", index))
            .absolute()
            .left(self.offset.x + px(place.x * z))
            .top(self.offset.y + px(place.y * z))
            .w(px(place.width * z))
            .h(px(place.height * z))
            .flex()
            .flex_col()
            .rounded(px(radius::RADIUS_SM * z.max(0.5)))
            .bg(theme::color(color::SURFACE))
            .border_1()
            .border_color(if focused { theme::color(color::BLUE) } else { theme::overlay(0.12) })
            .when(dimmed, |table| table.opacity(0.3))
            .overflow_hidden()
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(6. * z))
                    .h(px(HEADER_HEIGHT * z))
                    .px(px(10. * z))
                    .bg(theme::fill_hover())
                    .border_b_1()
                    .border_color(theme::separator())
                    .child(chrome::icon("database", 12. * z, theme::color(color::BLUE)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13. * z))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::color(color::INK))
                            .child(table.name.clone()),
                    ),
            )
            .child(div().flex().flex_col().pt(px(4. * z)).children(table.columns.iter().map(|column| {
                let (icon, tint) = match column.key {
                    Some(Key::Primary) => (Some("key-round"), color::AMBER_TEXT),
                    Some(Key::Foreign) => (Some("link-2"), color::BLUE),
                    Some(Key::Unique) => (Some("key-round"), color::INK_FAINT),
                    None => (None, color::INK_FAINT),
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(6. * z))
                    .h(px(ROW_HEIGHT * z))
                    .px(px(10. * z))
                    .child(
                        div()
                            .flex_none()
                            .w(px(12. * z))
                            .children(icon.map(|icon| chrome::icon(icon, 11. * z, theme::color(tint)))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12. * z))
                            .text_color(theme::color(color::INK_BODY))
                            .child(column.name.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(100. * z))
                            .truncate()
                            .font_family(saturn_design::font::MONO)
                            .text_size(px(11. * z))
                            .text_color(theme::color(color::INK_FAINT))
                            .child(format!("{}{}", column.ty, if column.nullable { "?" } else { "" })),
                    )
            })))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.focused = if this.focused == Some(index) { None } else { Some(index) };
                cx.notify();
            }))
            .into_any_element()
    }

    /// Zoom out, the zoom level, zoom in, and fit: floating in the corner.
    fn render_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().absolute().top(len(space::SPACE_2)).right(len(space::SPACE_2)).child(
            chrome::track()
                .bg(theme::color(color::SURFACE))
                .child(
                    chrome::icon_button("zoom-out", "zoom-out", true)
                        .on_click(cx.listener(|this, _, _, cx| this.zoom_centre(1. / 1.25, cx))),
                )
                .child(
                    div()
                        .w(px(44.))
                        .text_center()
                        .text_size(px(12.))
                        .text_color(theme::color(color::INK_MUTED))
                        .child(format!("{}%", (self.zoom * 100.).round())),
                )
                .child(
                    chrome::icon_button("zoom-in", "zoom-in", true)
                        .on_click(cx.listener(|this, _, _, cx| this.zoom_centre(1.25, cx))),
                )
                .child(chrome::track_separator())
                .child(chrome::icon_button("fit", "scan", true).on_click(cx.listener(|this, _, _, cx| {
                    this.fit();
                    cx.notify();
                }))),
        )
    }

    /// What the diagram was read from, bottom left.
    fn render_caption(&self) -> impl IntoElement {
        let sources = match self.schema.sources.as_slice() {
            [one] => one.display().to_string(),
            many => format!("{} schema files", many.len()),
        };
        div()
            .absolute()
            .left(len(space::SPACE_3))
            .bottom(len(space::SPACE_2))
            .text_size(px(11.))
            .text_color(theme::color(color::INK_FAINT))
            .child(format!(
                "{sources} · {} tables · {} relations · click a table to focus it",
                self.schema.tables.len(),
                self.schema.relations.len()
            ))
    }
}

/// The foreign keys, as curves from a column's row to the referenced column's row.
fn relations(view: &SchemaView, related: Option<&[bool]>) -> impl IntoElement {
    let z = view.zoom;
    let offset = view.offset;
    let bounds_cell = view.bounds.clone();
    let focused = view.focused.map(|index| view.schema.tables[index].name.clone());
    // (from point, to point, highlighted)
    let mut curves: Vec<(Point<f32>, Point<f32>, bool, bool)> = Vec::new();
    let find = |name: &str| view.schema.tables.iter().position(|table| table.name == name);
    for relation in &view.schema.relations {
        let (Some(from), Some(to)) = (find(&relation.from_table), find(&relation.to_table)) else { continue };
        let row = |table: usize, column: &str| {
            let position = view.schema.tables[table].columns.iter().position(|c| c.name == column).unwrap_or(0);
            HEADER_HEIGHT + 4. + ROW_HEIGHT * (f32::from(u16::try_from(position).unwrap_or(0)) + 0.5)
        };
        let (a, b) = (view.placed[from], view.placed[to]);
        let (ay, by) = (a.y + row(from, &relation.from_column), b.y + row(to, &relation.to_column));
        let rightward = a.x + a.width <= b.x;
        let (ax, bx) = if rightward {
            (a.x + a.width, b.x)
        } else if b.x + b.width <= a.x {
            (a.x, b.x + b.width)
        } else {
            (a.x + a.width, b.x + b.width)
        };
        let lit = focused.as_ref().is_some_and(|name| *name == relation.from_table || *name == relation.to_table);
        let dim = related.is_some() && !lit;
        curves.push((Point { x: ax, y: ay }, Point { x: bx, y: by }, lit, dim));
    }
    canvas(
        move |bounds, _, _| bounds_cell.set(Some(bounds)),
        move |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
            let at = |p: Point<f32>| {
                point(bounds.origin.x + offset.x + px(p.x * z), bounds.origin.y + offset.y + px(p.y * z))
            };
            for (from, to, lit, dim) in &curves {
                let (start, end) = (at(*from), at(*to));
                let reach = px((40. * z).max(16.)) * if end.x >= start.x { 1. } else { -1. };
                let same_side = (from.x - to.x).abs() < 1.;
                let (c1, c2) = if same_side {
                    (point(start.x + px(40. * z), start.y), point(end.x + px(40. * z), end.y))
                } else {
                    (point(start.x + reach, start.y), point(end.x - reach, end.y))
                };
                let mut path = PathBuilder::stroke(px(if *lit { 2. } else { 1.2 }));
                path.move_to(start);
                path.cubic_bezier_to(end, c1, c2);
                let colour: Hsla = if *lit {
                    theme::color(color::BLUE)
                } else if *dim {
                    theme::overlay(0.06)
                } else {
                    theme::overlay(0.28)
                };
                if let Ok(path) = path.build() {
                    window.paint_path(path, colour);
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl Render for SchemaView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.fit_pending && self.bounds.get().is_some() && self.loaded {
            self.fit();
        } else if self.fit_pending && self.loaded {
            // The view's size arrives with the first frame; fit on the next one.
            cx.on_next_frame(window, |_, _, cx| cx.notify());
        }
        if self.loaded && self.schema.tables.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.))
                .text_color(theme::color(color::INK_MUTED))
                .child("No database schema found: Saturn looks for Prisma schemas and SQL migrations.")
                .into_any_element();
        }
        let related = self.related();
        let mut root = div()
            .id("schema")
            .relative()
            .size_full()
            .overflow_hidden()
            .child(chrome::dot_grid())
            .child(relations(self, related.as_deref()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.dragging_from = Some(event.position);
                    this.focused = None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if let (Some(from), Some(MouseButton::Left)) = (this.dragging_from, event.pressed_button) {
                    this.offset += event.position - from;
                    this.dragging_from = Some(event.position);
                    cx.notify();
                } else {
                    this.dragging_from = None;
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.dragging_from = None))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(16.));
                if event.modifiers.platform || event.modifiers.control {
                    let factor = (1. + f32::from(delta.y) / 200.).clamp(0.5, 2.);
                    this.zoom_at(factor, event.position, cx);
                } else {
                    this.offset += delta;
                    cx.notify();
                }
            }))
            .on_pinch(cx.listener(|this, event: &PinchEvent, _, cx| {
                this.zoom_at(1. + event.delta, event.position, cx);
            }));
        for index in 0..self.schema.tables.len() {
            let dimmed = related.as_ref().is_some_and(|related| !related[index]);
            root = root.child(self.render_table(index, dimmed, cx));
        }
        root.child(self.render_controls(cx)).child(self.render_caption()).into_any_element()
    }
}
