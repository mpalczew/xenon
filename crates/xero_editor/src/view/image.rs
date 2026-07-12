//! Image zoom / pan handlers for `EditorView`.

use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, PinchEvent, ScrollWheelEvent, Window,
    point,
};

use super::{Content, EditorView, ZOOM_STEP};
use crate::image_viewer::{event_delta, zoom_factor_for_scroll};

impl EditorView {
    pub(super) fn zoom_image_in(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.zoom_by(ZOOM_STEP);
            cx.notify();
        }
    }

    pub(super) fn zoom_image_out(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.zoom_by(1. / ZOOM_STEP);
            cx.notify();
        }
    }

    pub(super) fn fit_image(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.fit();
            cx.notify();
        }
    }

    pub(super) fn actual_size_image(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.actual_size();
            cx.notify();
        }
    }

    pub(super) fn on_image_scroll(
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

    pub(super) fn on_image_pinch(
        &mut self,
        event: &PinchEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Image(viewer) = &mut self.content else {
            return;
        };
        viewer.zoom_at(1. + event.delta, event.position);
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn on_image_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Content::Image(viewer) = &mut self.content else {
            return;
        };
        viewer.drag_last = Some(event.position);
        self.focus.focus(window, cx);
    }

    pub(super) fn on_image_mouse_move(
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
        }
        let Some(last) = viewer.drag_last.replace(event.position) else {
            return;
        };
        viewer.pan(point(event.position.x - last.x, event.position.y - last.y));
        cx.stop_propagation();
        cx.notify();
    }
}
