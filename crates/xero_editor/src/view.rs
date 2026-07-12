//! `EditorView`: a focusable gpui view over a `Buffer`. Text input flows through
//! an `EntityInputHandler`; editing/navigation keys go through key-down; Cmd-S
//! saves. Mirrors the terminal view's input wiring.

mod disk;
mod image;
mod input;
mod layout;
mod menu;

use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
    StatefulInteractiveElement, Styled, Task, Window, anchored, deferred, div, px,
};
use theme::ActiveTheme;

use crate::buffer::{Buffer, OpenError};
use crate::element;
use crate::highlight;
use crate::image_viewer::{ImageContentElement, ImageViewer};
use crate::mouse::{ClickLayout, ClickTracker};
use crate::vim::VimState;
use menu::{context_item, file_title, is_supported_image};
use xero_settings::{Copy, Cut, Paste};

pub(super) const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;
pub(super) const ZOOM_STEP: f32 = 1.2;

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
    /// Poll disk mtime so agent/other-tool writes refresh a clean buffer.
    pub(super) _disk_poll: Task<()>,
    /// Clean buffer is fine; conflict/deleted when disk moved under dirty edits.
    pub(super) disk_alert: DiskAlert,
}

pub use disk::DiskAlert;

/// Events the shell can subscribe to (IDE selection push, etc.).
pub enum EditorEvent {
    SelectionChanged {
        path: PathBuf,
        text: String,
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    },
}

impl EventEmitter<EditorEvent> for EditorView {}

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
        Ok(cx.new(|cx| {
            let mut view = Self::from_content(content, autofocus, cx);
            view.start_disk_poll(cx);
            view
        }))
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
            _disk_poll: disk::idle_disk_poll(),
            disk_alert: DiskAlert::None,
        };
        view.recompute_highlights();
        view
    }

    /// Emit current selection to listeners (Claude IDE bridge).
    pub(super) fn emit_selection(&mut self, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let path = buffer.path().to_path_buf();
        let text = buffer.selected_text();
        let (start_off, end_off) = match buffer.selection_range() {
            Some(range) => (range.start, range.end),
            None => (buffer.cursor(), buffer.cursor()),
        };
        let start = crate::selection::offset_line_col(buffer.rope(), start_off);
        let end = crate::selection::offset_line_col(buffer.rope(), end_off);
        cx.emit(EditorEvent::SelectionChanged {
            path,
            text,
            start_line: start.0,
            start_character: start.1,
            end_line: end.0,
            end_character: end.1,
        });
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
    pub(super) fn recompute_highlights(&mut self) {
        self.highlights = match &self.content {
            Content::Text(buffer) => highlight::spans_for_path(buffer.path(), &buffer.text()),
            Content::Image(_) | Content::Unsupported { .. } => Vec::new(),
        };
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
        // Catch disk changes when this view paints (focus/tab switch).
        self.sync_from_disk(cx);
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
        let alert = self.disk_alert_bar(cx);
        if self.preview {
            return div()
                .track_focus(&self.focus)
                .key_context("Editor")
                .on_key_down(cx.listener(Self::on_key))
                .size_full()
                .flex()
                .flex_col()
                .bg(colors.editor_background)
                .children(alert)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(layout::markdown_preview(&self.text(), cx)),
                )
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
            .children(alert)
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
