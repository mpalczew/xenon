//! `EditorView`: a focusable gpui view over a `Buffer`. Text input flows through
//! an `EntityInputHandler`; editing/navigation keys go through key-down; Cmd-S
//! saves. Mirrors the terminal view's input wiring.

use std::ops::Range;
use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    AnyElement, App, AppContext, Bounds, Context, DispatchPhase, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, KeyDownEvent, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, ParentElement, PinchEvent, Pixels,
    Point, Render, ScrollDelta, ScrollWheelEvent, StatefulInteractiveElement, Style, Styled,
    StyledImage, UTF16Selection, Window, canvas, div, img, point, px, relative, size,
};
use image::ImageReader;
use theme::ActiveTheme;

use crate::buffer::{Buffer, OpenError};
use crate::edit::{EditCommand, Motion};
use crate::element::{self, ColoredSpan};
use crate::highlight;

const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;
const MIN_IMAGE_ZOOM: f32 = 0.05;
const MAX_IMAGE_ZOOM: f32 = 16.0;
const ZOOM_STEP: f32 = 1.2;

pub struct EditorView {
    content: Content,
    highlights: Vec<highlight::Span>,
    scroll_top: Pixels,
    scroll_left: Pixels,
    focus: FocusHandle,
    autofocus: bool,
    focused_once: bool,
    /// Render the markdown preview instead of the source (markdown files only).
    preview: bool,
    click_layout: Option<ClickLayout>,
}

enum Content {
    Text(Buffer),
    Image(ImageViewer),
    Unsupported { path: PathBuf, reason: String },
}

struct ImageViewer {
    path: PathBuf,
    natural_size: Option<ImageSize>,
    zoom: ImageZoom,
    pan_offset: Point<Pixels>,
    drag_last: Option<Point<Pixels>>,
    viewport: Option<Bounds<Pixels>>,
}

#[derive(Clone, Copy)]
struct ClickLayout {
    text_origin: Point<Pixels>,
    line_height: Pixels,
    scroll_top: Pixels,
    cell_width: Pixels,
}

#[derive(Clone, Copy)]
struct ImageSize {
    width: f32,
    height: f32,
}

#[derive(Clone, Copy)]
enum ImageZoom {
    Fit,
    Manual(f32),
}

impl EditorView {
    /// Open `path` and wrap it in an entity. `autofocus` grabs keyboard focus on
    /// first render (true for user-opened files, false for agent-opened ones so
    /// the terminal keeps focus).
    pub fn build(path: PathBuf, autofocus: bool, cx: &mut App) -> Result<Entity<Self>> {
        let content = match Buffer::open(&path) {
            Ok(buffer) => Content::Text(buffer),
            Err(OpenError::Binary(path)) if is_supported_image(&path) => {
                Content::Image(ImageViewer::new(path))
            }
            Err(OpenError::Binary(path)) => Content::Unsupported {
                path,
                reason: "xero cannot edit this binary file".into(),
            },
            Err(error) => return Err(error.into()),
        };
        Ok(cx.new(|cx| Self::from_content(content, autofocus, cx)))
    }

    fn from_content(content: Content, autofocus: bool, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            content,
            highlights: Vec::new(),
            scroll_top: px(0.),
            scroll_left: px(0.),
            focus: cx.focus_handle(),
            autofocus,
            focused_once: false,
            preview: false,
            click_layout: None,
        };
        view.recompute_highlights();
        view
    }

    /// The file this editor is showing.
    pub fn path(&self) -> &std::path::Path {
        match &self.content {
            Content::Text(buffer) => buffer.path(),
            Content::Image(viewer) => &viewer.path,
            Content::Unsupported { path, .. } => path,
        }
    }

    /// Whether this file can show a markdown preview.
    pub fn is_markdown(&self) -> bool {
        matches!(
            self.path().extension().and_then(|e| e.to_str()),
            Some("md" | "markdown" | "mdx")
        )
    }

    /// Toggle between source and rendered markdown (no-op for non-markdown).
    pub fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        if self.is_markdown() {
            self.preview = !self.preview;
            cx.notify();
        }
    }

    pub fn is_previewing(&self) -> bool {
        self.preview
    }

    fn on_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line_height =
            element::line_height(px(xero_settings::font_size(cx)), LINE_HEIGHT_MULTIPLIER);
        let delta = event.delta.pixel_delta(line_height);
        // Lower-bound only; the upper bound needs the viewport height, so `layout`
        // clamps it against the content each frame.
        self.scroll_top = (self.scroll_top - delta.y).max(px(0.));
        self.scroll_left = (self.scroll_left - delta.x).max(px(0.));
        cx.notify();
    }

    /// Re-highlight the whole buffer. Cheap enough for v1 file sizes; called
    /// after every edit. Unsupported languages get no spans (default color).
    fn recompute_highlights(&mut self) {
        self.highlights = match &self.content {
            Content::Text(buffer) => highlight::spans_for_path(buffer.path(), &buffer.text()),
            Content::Image(_) | Content::Unsupported { .. } => Vec::new(),
        };
    }

    fn zoom_image_in(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.zoom_by(ZOOM_STEP);
            cx.notify();
        }
    }

    fn zoom_image_out(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.zoom_by(1. / ZOOM_STEP);
            cx.notify();
        }
    }

    fn fit_image(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.zoom = ImageZoom::Fit;
            viewer.pan_offset = Point::default();
            cx.notify();
        }
    }

    fn actual_size_image(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.zoom = ImageZoom::Manual(1.);
            viewer.pan_offset = Point::default();
            cx.notify();
        }
    }

    fn on_image_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Image(viewer) = &mut self.content else {
            return;
        };
        if !event.modifiers.platform {
            viewer.pan(event_delta(event));
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let delta = event_delta(event).y;
        let factor = zoom_factor_for_scroll(delta);
        viewer.zoom_at(factor, event.position);
        cx.stop_propagation();
        cx.notify();
    }

    fn on_image_pinch(&mut self, event: &PinchEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Content::Image(viewer) = &mut self.content else {
            return;
        };
        viewer.zoom_at(1. + event.delta, event.position);
        cx.stop_propagation();
        cx.notify();
    }

    fn on_image_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Image(viewer) = &mut self.content else {
            return;
        };
        viewer.drag_last = Some(event.position);
        self.focus.focus(_window, cx);
    }

    fn on_image_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Image(viewer) = &mut self.content else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            viewer.drag_last = None;
            return;
        };
        let Some(last) = viewer.drag_last.replace(event.position) else {
            return;
        };
        viewer.pan(point(event.position.x - last.x, event.position.y - last.y));
        cx.stop_propagation();
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if matches!(self.content, Content::Image(_)) {
            if !keystroke.modifiers.platform {
                return;
            }
            match keystroke.key.as_str() {
                "=" | "+" => self.zoom_image_in(cx),
                "-" => self.zoom_image_out(cx),
                "0" => self.fit_image(cx),
                "1" => self.actual_size_image(cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if keystroke.modifiers.platform && keystroke.key == "s" {
            if let Err(error) = buffer.save() {
                log::error!("save failed: {error}");
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let Some(command) = command_for(&keystroke.key) else {
            return;
        };
        let edits = command.edits();
        buffer.apply(command);
        if edits {
            self.recompute_highlights();
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let Some(layout) = self.click_layout else {
            return;
        };
        let row = ((event.position.y - layout.text_origin.y + layout.scroll_top)
            / layout.line_height)
            .floor()
            .max(0.) as usize;
        let col = ((event.position.x - layout.text_origin.x) / layout.cell_width + 0.5)
            .floor()
            .max(0.) as usize;
        buffer.set_cursor_position(row, col);
        self.focus.focus(window, cx);
        cx.stop_propagation();
        cx.notify();
    }
}

fn command_for(key: &str) -> Option<EditCommand> {
    Some(match key {
        "backspace" => EditCommand::Backspace,
        "delete" => EditCommand::Delete,
        "enter" => EditCommand::Newline,
        "tab" => EditCommand::Insert("    ".into()),
        "left" => EditCommand::Move(Motion::Left),
        "right" => EditCommand::Move(Motion::Right),
        "up" => EditCommand::Move(Motion::Up),
        "down" => EditCommand::Move(Motion::Down),
        "home" => EditCommand::Move(Motion::LineStart),
        "end" => EditCommand::Move(Motion::LineEnd),
        _ => return None,
    })
}

impl Focusable for EditorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.autofocus && !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        let colors = cx.theme().colors();
        match &self.content {
            Content::Image(_) => return self.render_image(cx).into_any_element(),
            Content::Unsupported { path, reason } => {
                return self
                    .render_unsupported(path.clone(), reason.clone(), cx)
                    .into_any_element();
            }
            Content::Text(_) => {}
        }
        if self.preview {
            let size = px(xero_settings::font_size(cx));
            return div()
                .track_focus(&self.focus)
                .key_context("Editor")
                .on_key_down(cx.listener(Self::on_key))
                .size_full()
                .bg(colors.editor_background)
                .child(crate::markdown::render(&self.text(), size, cx))
                .into_any_element();
        }
        div()
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .size_full()
            .bg(colors.editor_background)
            .child(editor_canvas(cx.entity(), self.focus.clone()))
            .into_any_element()
    }
}

impl EditorView {
    fn render_image(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let Content::Image(_) = &mut self.content else {
            unreachable!("render_image is only called for image content");
        };
        div()
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.editor_background)
            .child(
                div()
                    .id("image-viewer")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .on_scroll_wheel(cx.listener(Self::on_image_scroll))
                    .on_pinch(cx.listener(Self::on_image_pinch))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_image_mouse_down))
                    .on_mouse_move(cx.listener(Self::on_image_mouse_move))
                    .child(ImageContentElement::new(cx.entity())),
            )
    }

    fn render_unsupported(
        &self,
        path: PathBuf,
        reason: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let title = file_title(&path);
        div()
            .track_focus(&self.focus)
            .key_context("Editor")
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.editor_background)
            .child(file_header(path.clone(), title, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .text_color(colors.text_muted)
                            .child(
                                div()
                                    .text_lg()
                                    .text_color(colors.text)
                                    .child(file_title(&path)),
                            )
                            .child(reason),
                    ),
            )
    }

    fn text(&self) -> String {
        match &self.content {
            Content::Text(buffer) => buffer.text(),
            Content::Image(_) | Content::Unsupported { .. } => String::new(),
        }
    }
}

impl ImageViewer {
    fn new(path: PathBuf) -> Self {
        Self {
            natural_size: read_image_size(&path),
            path,
            zoom: ImageZoom::Fit,
            pan_offset: Point::default(),
            drag_last: None,
            viewport: None,
        }
    }

    fn zoom_by(&mut self, factor: f32) {
        let zoom = self.current_zoom() * factor;
        self.zoom = ImageZoom::Manual(clamp_zoom(zoom));
        self.clamp_pan();
    }

    fn zoom_at(&mut self, factor: f32, position: Point<Pixels>) {
        let old_zoom = self.current_zoom();
        let new_zoom = clamp_zoom(old_zoom * factor);
        self.zoom = ImageZoom::Manual(new_zoom);
        self.anchor_zoom(old_zoom, new_zoom, position);
    }

    fn pan(&mut self, delta: Point<Pixels>) {
        self.pan_offset += delta;
        self.clamp_pan();
    }

    fn current_zoom(&self) -> f32 {
        match self.zoom {
            ImageZoom::Fit => self.fit_zoom(),
            ImageZoom::Manual(zoom) => zoom,
        }
    }

    fn fit_zoom(&self) -> f32 {
        let Some(size) = self.natural_size else {
            return 1.;
        };
        let Some(viewport) = self.viewport.map(|bounds| bounds.size) else {
            return 1.;
        };
        if viewport.width.as_f32() <= 0. || viewport.height.as_f32() <= 0. {
            return 1.;
        }
        (viewport.width.as_f32() / size.width)
            .min(viewport.height.as_f32() / size.height)
            .clamp(MIN_IMAGE_ZOOM, 1.)
    }

    fn scaled_size(&self, zoom: f32) -> ImageSize {
        self.natural_size
            .map(|size| ImageSize {
                width: size.width * zoom,
                height: size.height * zoom,
            })
            .unwrap_or(ImageSize {
                width: 1.,
                height: 1.,
            })
    }

    fn zoom_for_bounds(&self, bounds: Bounds<Pixels>) -> f32 {
        match self.zoom {
            ImageZoom::Fit => fit_zoom_for(bounds, self.natural_size),
            ImageZoom::Manual(zoom) => zoom,
        }
    }

    fn clamp_pan(&mut self) {
        if let Some(bounds) = self.viewport {
            self.pan_offset = clamp_pan(
                self.pan_offset,
                bounds,
                self.natural_size,
                self.current_zoom(),
            );
        }
    }

    fn anchor_zoom(&mut self, old_zoom: f32, new_zoom: f32, position: Point<Pixels>) {
        let Some(bounds) = self.viewport else {
            self.clamp_pan();
            return;
        };
        let relative_center = point(
            position.x - bounds.origin.x - bounds.size.width / 2.,
            position.y - bounds.origin.y - bounds.size.height / 2.,
        );
        let offset_from_image = relative_center - self.pan_offset;
        let ratio = new_zoom / old_zoom;
        self.pan_offset += offset_from_image * (1. - ratio);
        self.pan_offset = clamp_pan(self.pan_offset, bounds, self.natural_size, new_zoom);
    }
}

fn event_delta(event: &ScrollWheelEvent) -> Point<Pixels> {
    match event.delta {
        ScrollDelta::Pixels(delta) => delta,
        ScrollDelta::Lines(delta) => delta.map(|value| px(value * 20.)),
    }
}

fn zoom_factor_for_scroll(delta: Pixels) -> f32 {
    let delta: f32 = delta.into();
    if delta > 0. {
        1. + delta.abs() * 0.01
    } else {
        1. / (1. + delta.abs() * 0.01)
    }
}

struct ImageContentElement {
    view: Entity<EditorView>,
}

impl ImageContentElement {
    fn new(view: Entity<EditorView>) -> Self {
        Self { view }
    }
}

impl IntoElement for ImageContentElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ImageContentElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<(AnyElement, bool)>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (
            window.request_layout(
                Style {
                    size: size(relative(1.).into(), relative(1.).into()),
                    ..Default::default()
                },
                [],
                cx,
            ),
            (),
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let (path, pan, size, dragging) = {
            let view = self.view.read(cx);
            let Content::Image(viewer) = &view.content else {
                return None;
            };
            let zoom = viewer.zoom_for_bounds(bounds);
            (
                viewer.path.clone(),
                clamp_pan(viewer.pan_offset, bounds, viewer.natural_size, zoom),
                viewer.scaled_size(zoom),
                viewer.drag_last.is_some(),
            )
        };
        self.view.update(cx, |view, _| {
            if let Content::Image(viewer) = &mut view.content {
                viewer.viewport = Some(bounds);
                viewer.pan_offset = pan;
            }
        });

        let left = bounds.size.width / 2. - px(size.width) / 2. + pan.x;
        let top = bounds.size.height / 2. - px(size.height) / 2. + pan.y;
        let mut image = div()
            .relative()
            .size_full()
            .child(
                div()
                    .absolute()
                    .left(left)
                    .top(top)
                    .w(px(size.width))
                    .h(px(size.height))
                    .child(img(path).object_fit(ObjectFit::Fill).size_full()),
            )
            .into_any_element();

        image.prepaint_as_root(bounds.origin, bounds.size.into(), window, cx);
        Some((image, dragging))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some((mut element, dragging)) = prepaint.take() else {
            return;
        };
        if dragging {
            let view = self.view.clone();
            window.on_mouse_event(move |_event: &MouseUpEvent, phase, _window, cx| {
                if phase == DispatchPhase::Bubble {
                    view.update(cx, |view, cx| {
                        if let Content::Image(viewer) = &mut view.content {
                            viewer.drag_last = None;
                            cx.notify();
                        }
                    });
                }
            });
        }
        element.paint(window, cx);
    }
}

fn clamp_pan(
    pan: Point<Pixels>,
    bounds: Bounds<Pixels>,
    natural_size: Option<ImageSize>,
    zoom: f32,
) -> Point<Pixels> {
    let Some(size) = natural_size else {
        return Point::default();
    };
    let overflow_x = (px(size.width * zoom) - bounds.size.width).max(px(0.)) / 2.;
    let overflow_y = (px(size.height * zoom) - bounds.size.height).max(px(0.)) / 2.;
    point(
        pan.x.max(-overflow_x).min(overflow_x),
        pan.y.max(-overflow_y).min(overflow_y),
    )
}

fn fit_zoom_for(bounds: Bounds<Pixels>, natural_size: Option<ImageSize>) -> f32 {
    let Some(size) = natural_size else {
        return 1.;
    };
    if bounds.size.width.as_f32() <= 0. || bounds.size.height.as_f32() <= 0. {
        return 1.;
    }
    (bounds.size.width.as_f32() / size.width)
        .min(bounds.size.height.as_f32() / size.height)
        .clamp(MIN_IMAGE_ZOOM, 1.)
}

fn read_image_size(path: &std::path::Path) -> Option<ImageSize> {
    let size = ImageReader::open(path).ok()?.into_dimensions().ok()?;
    Some(ImageSize {
        width: size.0 as f32,
        height: size.1 as f32,
    })
}

fn clamp_zoom(zoom: f32) -> f32 {
    zoom.clamp(MIN_IMAGE_ZOOM, MAX_IMAGE_ZOOM)
}

fn file_header(path: PathBuf, title: String, cx: &mut Context<EditorView>) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    div()
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(colors.border)
        .px_3()
        .py_2()
        .child(
            div()
                .text_sm()
                .text_color(colors.text)
                .truncate()
                .child(title),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(native_button(path.clone()))
                .child(reveal_button(path, cx)),
        )
}

fn native_button(path: PathBuf) -> impl IntoElement {
    small_button("native-open", "Open in Default App")
        .on_click(move |_, _, cx| cx.open_with_system(&path))
}

fn reveal_button(path: PathBuf, _cx: &mut Context<EditorView>) -> impl IntoElement {
    small_button("native-reveal", "Reveal").on_click(move |_, _, cx| cx.reveal_path(&path))
}

fn small_button(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .text_xs()
        .rounded_sm()
        .border_1()
        .cursor_pointer()
        .child(label)
}

fn editor_canvas(view: Entity<EditorView>, focus: FocusHandle) -> impl IntoElement {
    let font = element::editor_font();
    canvas(
        {
            let view = view.clone();
            move |bounds, window, cx| layout(&view, &font, bounds, window, cx)
        },
        move |bounds, editor_layout, window, cx| {
            let cursor_color = cx.theme().players().local().cursor;
            element::paint(&editor_layout, cursor_color, window, cx);
            window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .size_full()
}

fn layout(
    view: &Entity<EditorView>,
    font: &gpui::Font,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> element::EditorLayout {
    let size = px(xero_settings::font_size(cx));
    let line_height = element::line_height(size, LINE_HEIGHT_MULTIPLIER);
    let lines = match &view.read(cx).content {
        Content::Text(buffer) => buffer.rope().len_lines(),
        Content::Image(_) | Content::Unsupported { .. } => 0,
    };
    let content_height = line_height * (lines as f32);
    let max_scroll = (content_height - bounds.size.height).max(px(0.));
    view.update(cx, |view, _| {
        view.scroll_top = view.scroll_top.min(max_scroll)
    });

    let show_line_numbers = xero_settings::show_line_numbers(cx);
    let editor_layout = {
        let view = view.read(cx);
        let theme = cx.theme();
        let text_color = theme.colors().editor_foreground;
        let syntax = theme.syntax();
        let spans: Vec<ColoredSpan> = view
            .highlights
            .iter()
            .map(|span| {
                let color = syntax
                    .style_for_name(span.name)
                    .and_then(|style| style.color)
                    .unwrap_or(text_color);
                ColoredSpan {
                    start: span.start,
                    end: span.end,
                    color,
                }
            })
            .collect();
        let Content::Text(buffer) = &view.content else {
            unreachable!("non-text editor content does not render the text canvas");
        };
        element::layout(
            element::LayoutInput {
                rope: buffer.rope(),
                cursor: buffer.cursor_position(),
                default_color: text_color,
                line_number_color: theme.colors().text_muted,
                gutter_color: theme.colors().panel_background,
                scrollbar_color: theme.colors().text_muted,
                spans: &spans,
                origin: bounds.origin,
                viewport_width: bounds.size.width,
                viewport_height: bounds.size.height,
                scroll_top: view.scroll_top,
                scroll_left: view.scroll_left,
                show_line_numbers,
            },
            element::TextMetrics {
                font,
                font_size: size,
                line_height,
            },
            window,
        )
    };
    view.update(cx, |view, _| {
        view.scroll_left = editor_layout.scroll_left;
        view.click_layout = Some(ClickLayout {
            text_origin: editor_layout.text_origin,
            line_height: editor_layout.line_height,
            scroll_top: editor_layout.scroll_top,
            cell_width: editor_layout.cell_width,
        });
    });
    editor_layout
}

impl EntityInputHandler for EditorView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if !text.is_empty() {
            buffer.apply(EditCommand::Insert(text.to_string()));
            self.recompute_highlights();
            cx.notify();
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        if !new_text.is_empty() {
            buffer.apply(EditCommand::Insert(new_text.to_string()));
            self.recompute_highlights();
            cx.notify();
        }
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

fn is_supported_image(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| gpui::Img::extensions().contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn file_title(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
