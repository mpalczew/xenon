//! Image zoom / pan handlers and non-text editor chrome for `EditorView`.

use std::path::PathBuf;

use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, PinchEvent, ScrollWheelEvent, StatefulInteractiveElement, Styled, Window, div,
    point,
};
use theme::ActiveTheme;

use super::menu::file_title;
use super::{Content, EditorView, ZOOM_STEP};
use crate::image_viewer::{ImageContentElement, event_delta, zoom_factor_for_scroll};

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

    pub(super) fn render_image(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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

    pub(super) fn render_unsupported(
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

    pub(super) fn text(&self) -> String {
        match &self.content {
            Content::Text(buffer) => buffer.text(),
            Content::Image(_) | Content::Unsupported { .. } => String::new(),
        }
    }
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
                .child(reveal_button(path)),
        )
}

fn native_button(path: PathBuf) -> impl IntoElement {
    small_button("native-open", "Open in Default App")
        .on_click(move |_, _, cx| cx.open_with_system(&path))
}

fn reveal_button(path: PathBuf) -> impl IntoElement {
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
