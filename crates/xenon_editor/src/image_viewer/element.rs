//! GPUI paint element for the image tab (zoom/pan layout).

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Element, ElementId, Entity, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, MouseUpEvent, ObjectFit, ParentElement, Pixels,
    Style, Styled, StyledImage, Window, div, img, px, relative, size,
};

use super::clamp_pan;
use crate::view::{Content, EditorView};

pub(crate) struct ImageContentElement {
    view: Entity<EditorView>,
}

impl ImageContentElement {
    pub(crate) fn new(view: Entity<EditorView>) -> Self {
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
        let (image, pan, size, dragging) = {
            let view = self.view.read(cx);
            let Content::Image(viewer) = &view.content else {
                return None;
            };
            let zoom = viewer.zoom_for_bounds(bounds);
            (
                viewer.image.clone(),
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

        let image = image?;

        let left = bounds.size.width / 2. - px(size.width) / 2. + pan.x;
        let top = bounds.size.height / 2. - px(size.height) / 2. + pan.y;
        let mut image_el = div()
            .relative()
            .size_full()
            .child(
                div()
                    .absolute()
                    .left(left)
                    .top(top)
                    .w(px(size.width))
                    .h(px(size.height))
                    .child(img(image).object_fit(ObjectFit::Fill).size_full()),
            )
            .into_any_element();

        image_el.prepaint_as_root(bounds.origin, bounds.size.into(), window, cx);
        Some((image_el, dragging))
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
