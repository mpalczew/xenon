//! `EditorView`: a focusable gpui view over a `Buffer`. Text input flows through
//! an `EntityInputHandler`; editing/navigation keys go through key-down; Cmd-S
//! saves. Mirrors the terminal view's input wiring.

mod disk;
mod entity_input;
mod ex_io;
mod find_bar;
mod find_session;
mod image;
mod input;
mod layout;
mod lsp;
mod menu;

use std::ops::Range;
use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, Render,
    ScrollWheelEvent, Styled, Subscription, Task, Window, actions, div, px,
};
use theme::ActiveTheme;

use crate::buffer::{Buffer, OpenError};
use crate::element;
use crate::highlight;
use crate::image_viewer::ImageViewer;
use crate::markdown::{PreviewEvent, PreviewState};
use crate::mouse::{ClickLayout, ClickTracker};
use crate::vim::VimState;
use find_session::FindSession;
use menu::is_supported_image;
use xenon_settings::{Copy, Cut, Paste, SelectAll};

actions!(xenon_editor, [Find, FindNext, FindPrevious]);

pub(super) const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;
pub(super) const ZOOM_STEP: f32 = 1.2;

pub struct EditorView {
    pub(super) content: Content,
    pub(super) highlights: Vec<highlight::Span>,
    pub(super) occurrence_ranges: Vec<Range<usize>>,
    pub(super) diagnostics: Vec<EditorDiagnostic>,
    pub(super) lsp_status: Option<String>,
    pub(super) scroll_top: Pixels,
    pub(super) scroll_left: Pixels,
    /// Last laid-out cursor; when it changes, layout scrolls to follow.
    pub(super) last_cursor: Option<(usize, usize)>,
    focus: FocusHandle,
    autofocus: bool,
    focused_once: bool,
    /// Render the markdown preview instead of the source (markdown files only).
    preview: bool,
    /// Selection host for markdown preview (plain blocks + carets).
    preview_state: Entity<PreviewState>,
    _preview_sel_sub: Subscription,
    pub(super) click_layout: Option<ClickLayout>,
    click_tracker: ClickTracker,
    dragging: bool,
    /// Right-click Cut/Copy/Paste menu position (window coords), when open.
    context_menu: Option<Point<Pixels>>,
    vim: VimState,
    /// cmd-f find bar (independent of vim `/`).
    find: Option<FindSession>,
    /// Poll disk mtime so agent/other-tool writes refresh a clean buffer.
    pub(super) _disk_poll: Task<()>,
    /// Clean buffer is fine; conflict/deleted when disk moved under dirty edits.
    pub(super) disk_alert: DiskAlert,
}

pub use disk::DiskAlert;

/// Events the shell can subscribe to (IDE selection push, ex quit, etc.).
pub enum EditorEvent {
    SelectionChanged {
        path: PathBuf,
        text: String,
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    },
    /// Vim `:q` / `:wq` — close this editor tab.
    RequestClose {
        force: bool,
    },
    /// Buffer path rebinding (Save As / `:w path`).
    PathChanged {
        path: PathBuf,
    },
    BufferChanged {
        path: PathBuf,
        text: String,
    },
    Saved {
        path: PathBuf,
    },
    CursorMoved {
        path: PathBuf,
        row: u32,
        col: u32,
    },
    GoToDefinition {
        path: PathBuf,
        row: u32,
        col: u32,
    },
}

impl EventEmitter<EditorEvent> for EditorView {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct EditorDiagnostic {
    pub range: Range<usize>,
    pub severity: EditorDiagnosticSeverity,
    pub message: String,
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
                reason: "xenon cannot edit this binary file".into(),
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
        let preview_state = cx.new(|_| PreviewState::default());
        let _preview_sel_sub =
            cx.subscribe(&preview_state, |_, _, _: &PreviewEvent, cx| cx.notify());
        let mut view = Self {
            content,
            highlights: Vec::new(),
            occurrence_ranges: Vec::new(),
            diagnostics: Vec::new(),
            lsp_status: None,
            scroll_top: px(0.),
            scroll_left: px(0.),
            last_cursor: None,
            focus: cx.focus_handle(),
            autofocus,
            focused_once: false,
            preview: false,
            preview_state,
            _preview_sel_sub,
            click_layout: None,
            click_tracker: ClickTracker::default(),
            dragging: false,
            context_menu: None,
            vim: VimState::default(),
            find: None,
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

    /// ⌘A — select entire buffer (or all rendered preview text when previewing).
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            self.preview_state
                .update(cx, |state, cx| state.select_all(cx));
            cx.notify();
            return;
        }
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let len = buffer.rope().len_chars();
        buffer.set_selection(0, len);
        self.emit_selection(cx);
        cx.notify();
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
            self.preview_state
                .update(cx, |state, cx| state.clear_selection(cx));
            self.context_menu = None;
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
        let size = px(xenon_settings::editor_font(cx).size);
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
        if self.find_is_open() {
            self.rescan_find();
        }
    }
}

impl Focusable for EditorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EditorView {
    fn render_preview(
        &mut self,
        alert: Option<impl IntoElement + 'static>,
        colors: &theme::ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let menu = self
            .context_menu
            .map(|position| self.render_preview_context_menu(position, colors, cx));
        let rendered = layout::markdown_preview(&self.text(), self.preview_state.clone(), cx);
        self.preview_state.update(cx, |state, cx| {
            state.sync_doc(
                rendered.source,
                rendered.plain_blocks,
                rendered.source_ranges,
                cx,
            );
        });
        div()
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                this.copy_selection(cx);
            }))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.select_all(cx);
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.editor_background)
            .children(alert)
            .child(div().flex_1().min_h_0().child(rendered.element))
            .children(menu)
            .into_any_element()
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
            return self.render_preview(alert, &colors, cx);
        }
        let mode_bar = self.vim_mode_bar(&colors, cx);
        let menu = self
            .context_menu
            .map(|position| self.render_context_menu(position, &colors, cx));
        let find_bar = self.render_find_bar(&colors, cx);
        let lsp_status = self.lsp_status.clone().map(|status| {
            div()
                .px_2()
                .py_px()
                .text_xs()
                .bg(colors.panel_background)
                .text_color(colors.text_muted)
                .child(status)
        });
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
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.select_all(cx);
            }))
            .on_action(cx.listener(|this, _: &Find, window, cx| {
                this.open_find(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, _, cx| {
                this.find_next(cx);
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, _, cx| {
                this.find_previous(cx);
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
            .children(lsp_status)
            .children(find_bar)
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
