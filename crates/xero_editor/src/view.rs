//! `EditorView`: a focusable gpui view over a `Buffer`. Text input flows through
//! an `EntityInputHandler`; editing/navigation keys go through key-down; Cmd-S
//! saves. Mirrors the terminal view's input wiring.

mod input;
mod layout;

use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, PinchEvent, Pixels, Point, Render,
    ScrollWheelEvent, StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point,
    px,
};
use theme::ActiveTheme;

use crate::buffer::{Buffer, OpenError};
use crate::element;
use crate::highlight;
use crate::image_viewer::{ImageContentElement, ImageViewer, event_delta, zoom_factor_for_scroll};
use crate::mouse::{ClickLayout, ClickTracker};
use crate::vim::VimState;
use xero_settings::{Copy, Cut, Paste};

pub(super) const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;
const ZOOM_STEP: f32 = 1.2;

pub struct EditorView {
    pub(super) content: Content,
    pub(super) highlights: Vec<highlight::Span>,
    pub(super) scroll_top: Pixels,
    pub(super) scroll_left: Pixels,
    /// Last laid-out cursor; when it changes, layout scrolls to follow.
    pub(super) last_cursor: Option<(usize, usize)>,
    focus: FocusHandle,
    autofocus: bool,
    focused_once: bool,
    /// Render the markdown preview instead of the source (markdown files only).
    preview: bool,
    pub(super) click_layout: Option<ClickLayout>,
    click_tracker: ClickTracker,
    dragging: bool,
    /// Right-click Cut/Copy/Paste menu position (window coords), when open.
    context_menu: Option<Point<Pixels>>,
    vim: VimState,
}

pub(super) enum Content {
    Text(Buffer),
    Image(ImageViewer),
    Unsupported { path: PathBuf, reason: String },
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
            last_cursor: None,
            focus: cx.focus_handle(),
            autofocus,
            focused_once: false,
            preview: false,
            click_layout: None,
            click_tracker: ClickTracker::default(),
            dragging: false,
            context_menu: None,
            vim: VimState::default(),
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
        let size = px(xero_settings::editor_font(cx).size);
        let line_height = element::line_height(size, LINE_HEIGHT_MULTIPLIER);
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
            viewer.fit();
            cx.notify();
        }
    }

    fn actual_size_image(&mut self, cx: &mut Context<Self>) {
        if let Content::Image(viewer) = &mut self.content {
            viewer.actual_size();
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
        let colors = cx.theme().colors().clone();
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
            return div()
                .track_focus(&self.focus)
                .key_context("Editor")
                .on_key_down(cx.listener(Self::on_key))
                .size_full()
                .bg(colors.editor_background)
                .child(layout::markdown_preview(&self.text(), cx))
                .into_any_element();
        }
        let mode_bar = xero_settings::vim_mode(cx).then(|| {
            let label = if let Some(draft) = &self.vim.search_draft {
                let prefix = if draft.forward { '/' } else { '?' };
                format!("{prefix}{}", draft.pattern)
            } else {
                self.vim.mode.label().to_string()
            };
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(colors.text_muted)
                .border_t_1()
                .border_color(colors.border)
                .child(label)
        });
        let menu = self
            .context_menu
            .map(|position| self.render_context_menu(position, &colors, cx));
        div()
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                this.cut_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                this.copy_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                this.paste_clipboard(cx);
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.editor_background)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(layout::editor_canvas(cx.entity(), self.focus.clone())),
            )
            .children(mode_bar)
            .children(menu)
            .into_any_element()
    }
}

impl EditorView {
    fn render_context_menu(
        &self,
        position: Point<Pixels>,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .child(
                context_item("editor-menu-cut", "Cut", "⌘X", colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.cut_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("editor-menu-copy", "Copy", "⌘C", colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.copy_selection(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            )
            .child(
                context_item("editor-menu-paste", "Paste", "⌘V", colors).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.paste_clipboard(cx);
                        this.dismiss_menu(cx);
                    },
                )),
            );
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| this.dismiss_menu(cx)),
            )
            .child(deferred(anchored().position(position).child(menu_box)).with_priority(1))
    }

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

/// Context-menu row: label on the left, keybinding on the right.
fn context_item(
    id: &'static str,
    label: &'static str,
    shortcut: &'static str,
    colors: &theme::ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    let hover = colors.element_hover;
    let muted = colors.text_muted;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .px_3()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label)
        .child(div().text_xs().text_color(muted).child(shortcut))
}
