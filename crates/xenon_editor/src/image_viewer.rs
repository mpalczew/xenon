use std::path::PathBuf;

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Element, ElementId, Entity, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, MouseUpEvent, ObjectFit, ParentElement, Pixels,
    Point, ScrollDelta, ScrollWheelEvent, Style, Styled, StyledImage, Window, div, img, point, px,
    relative, size,
};
use image::ImageReader;

use crate::view::{Content, EditorView};

const MIN_IMAGE_ZOOM: f32 = 0.05;
const MAX_IMAGE_ZOOM: f32 = 16.0;

pub(super) struct ImageViewer {
    pub(super) path: PathBuf,
    natural_size: Option<ImageSize>,
    zoom: ImageZoom,
    pub(super) pan_offset: Point<Pixels>,
    pub(super) drag_last: Option<Point<Pixels>>,
    pub(super) viewport: Option<Bounds<Pixels>>,
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

impl ImageViewer {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            natural_size: read_image_size(&path),
            path,
            zoom: ImageZoom::Fit,
            pan_offset: Point::default(),
            drag_last: None,
            viewport: None,
        }
    }

    pub(super) fn zoom_by(&mut self, factor: f32) {
        let zoom = self.current_zoom() * factor;
        self.zoom = ImageZoom::Manual(clamp_zoom(zoom));
        self.clamp_pan();
    }

    pub(super) fn zoom_at(&mut self, factor: f32, position: Point<Pixels>) {
        let old_zoom = self.current_zoom();
        let new_zoom = clamp_zoom(old_zoom * factor);
        self.zoom = ImageZoom::Manual(new_zoom);
        self.anchor_zoom(old_zoom, new_zoom, position);
    }

    pub(super) fn pan(&mut self, delta: Point<Pixels>) {
        self.pan_offset += delta;
        self.clamp_pan();
    }

    pub(super) fn fit(&mut self) {
        self.zoom = ImageZoom::Fit;
        self.pan_offset = Point::default();
    }

    pub(super) fn actual_size(&mut self) {
        self.zoom = ImageZoom::Manual(1.);
        self.pan_offset = Point::default();
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

pub(super) fn event_delta(event: &ScrollWheelEvent) -> Point<Pixels> {
    match event.delta {
        ScrollDelta::Pixels(delta) => delta,
        ScrollDelta::Lines(delta) => delta.map(|value| px(value * 20.)),
    }
}

pub(super) fn zoom_factor_for_scroll(delta: Pixels) -> f32 {
    let delta: f32 = delta.into();
    if delta > 0. {
        1. + delta.abs() * 0.01
    } else {
        1. / (1. + delta.abs() * 0.01)
    }
}

pub(super) struct ImageContentElement {
    view: Entity<EditorView>,
}

impl ImageContentElement {
    pub(super) fn new(view: Entity<EditorView>) -> Self {
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
