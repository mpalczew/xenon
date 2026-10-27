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
mod links;
mod lsp;
mod menu;
mod prose;
mod remote;
mod selected;
#[cfg(feature = "visual-tests")]
mod visual;
mod worklist;

use std::ops::Range;
use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, Render, ScrollWheelEvent, Styled,
    Subscription, Task, Window, actions, div, px,
};
use theme::ActiveTheme;

use crate::buffer::{Buffer, OpenError};
use crate::element;
use crate::highlight;
use crate::image_viewer::ImageViewer;
use crate::markdown::LinkTarget;
use crate::mouse::{ClickLayout, ClickTracker};
use crate::vim::VimState;
use find_session::FindSession;
use menu::is_supported_image;
use xenon_design_system::CursorBlink;
use xenon_settings::{Copy, Cut, Paste, SelectAll};

actions!(
    xenon_editor,
    [
        Find,
        FindNext,
        FindPrevious,
        ProseMode,
        SplitMode,
        SourceMode,
        TogglePaper,
        ToggleFullScreen,
        ToggleTask
    ]
);

pub(super) const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;
pub(super) const ZOOM_STEP: f32 = 1.2;

pub struct EditorView {
    remote: Option<remote::RemoteDocument>,
    pub(super) content: Content,
    pub(super) highlights: Vec<highlight::Span>,
    pub(super) occurrence_ranges: Vec<Range<usize>>,
    pub(super) diagnostics: Vec<EditorDiagnostic>,
    pub(super) lsp_status: Option<String>,
    pub(super) scroll_top: Pixels,
    pub(super) scroll_left: Pixels,
    /// Last laid-out cursor; when it changes, layout scrolls to follow.
    pub(super) last_cursor: Option<(usize, usize)>,
    /// Next layout should center the cursor line (`zz`).
    pub(super) scroll_center_once: bool,
    focus: FocusHandle,
    autofocus: bool,
    focused_once: bool,
    cursor_blink: CursorBlink,
    /// Markdown prose mode state (mode, Paper, layout cache).
    pub(super) prose: prose::ProseUi,
    worklist_document: bool,
    worklist_raw: bool,
    worklist_cursor: worklist::Cursor,
    /// Folded section titles: view state for this session, never written to the file.
    worklist_folded: std::collections::BTreeSet<String>,
    worklist_section_edit: Option<worklist::SectionEdit>,
    /// Where an Add in progress files its item.
    worklist_add_target: crate::worklist_file::Target,
    /// Take focus back on the next paint, after an inline field closes.
    worklist_refocus: bool,
    worklist_edit: Option<Entity<crate::item_editor::ItemEditor>>,
    worklist_input_sub: Vec<Subscription>,
    worklist_error: Option<String>,
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
    /// Mouse (or other user action) claimed keyboard focus on this view.
    Focused,
    /// Prose full screen was toggled: the shell hides or restores its chrome.
    FullScreen {
        on: bool,
    },
    /// A markdown link to a file or URL was followed from `path`.
    OpenLink {
        path: PathBuf,
        target: LinkTarget,
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
    /// Open or initialize the virtual worklist document without creating its file.
    pub fn build_worklist(path: PathBuf, autofocus: bool, cx: &mut App) -> Result<Entity<Self>> {
        let exists = path.exists();
        let buffer = if exists {
            Buffer::open(&path)?
        } else {
            Buffer::empty(&path)
        };
        Ok(cx.new(|cx| {
            let mut view = Self::from_content(Content::Text(buffer), autofocus, cx);
            view.worklist_document = true;
            view.start_disk_poll(cx);
            view
        }))
    }

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
        let mut view = Self {
            remote: None,
            content,
            highlights: Vec::new(),
            occurrence_ranges: Vec::new(),
            diagnostics: Vec::new(),
            lsp_status: None,
            scroll_top: px(0.),
            scroll_left: px(0.),
            scroll_center_once: false,
            last_cursor: None,
            focus: cx.focus_handle(),
            autofocus,
            focused_once: false,
            cursor_blink: CursorBlink::default(),
            prose: Default::default(),
            worklist_document: false,
            worklist_raw: false,
            worklist_cursor: worklist::Cursor::Item(0),
            worklist_folded: Default::default(),
            worklist_section_edit: None,
            worklist_add_target: Default::default(),
            worklist_refocus: false,
            worklist_edit: None,
            worklist_input_sub: Vec::new(),
            worklist_error: None,
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

    fn blink_tick(&mut self, generation: u64, cx: &mut Context<Self>) -> bool {
        let active = self.cursor_blink.tick(generation);
        if active {
            cx.notify();
        }
        active
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
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        let len = buffer.rope().len_chars();
        buffer.set_selection(0, len);
        self.emit_selection(cx);
        cx.notify();
    }

    pub fn set_selection_range(
        &mut self,
        start: (u32, u32),
        end: (u32, u32),
        cx: &mut Context<Self>,
    ) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.set_cursor_position(start.0 as usize, start.1 as usize);
        buffer.set_cursor_position_extend(end.0 as usize, end.1 as usize, true);
        self.emit_cursor(cx);
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

    fn is_markdown(&self) -> bool {
        matches!(
            self.path().extension().and_then(|e| e.to_str()),
            Some("md" | "markdown" | "mdx")
        )
    }

    /// Toggle between Prose and Source for markdown (no-op elsewhere).
    pub fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        let to = if self.markdown_mode() == xenon_settings::MarkdownMode::Source {
            xenon_settings::MarkdownMode::Prose
        } else {
            xenon_settings::MarkdownMode::Source
        };
        self.set_markdown_mode(to, cx);
    }

    /// Whether a rendered (prose) pane is showing.
    pub fn is_previewing(&self) -> bool {
        self.shows_prose_pane()
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

impl Render for EditorView {
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.autofocus && !self.focused_once {
            self.focus.focus(window, cx);
            self.focused_once = true;
        }
        self.sync_from_disk(cx);
        self.ensure_prose_prefs();
        let cursor_focused = self.focus.is_focused(window)
            && window.is_window_active()
            && matches!(self.content, Content::Text(_))
            && (!self.worklist_document || self.worklist_raw);
        self.cursor_blink
            .update_focus(cursor_focused, cx, Self::blink_tick);
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
        if self.worklist_document && !self.worklist_raw {
            return self.render_worklist(window, cx).into_any_element();
        }
        let alert = self.disk_alert_bar(cx);
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
        let markdown = self.is_md_prose_capable();
        let strip = self.prose_strip(cx);
        let prose_active = self.prose_active();
        let source = self.shows_source_pane().then(|| {
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                .on_mouse_move(cx.listener(Self::on_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                .on_scroll_wheel(cx.listener(Self::on_scroll))
                .child(layout::editor_canvas(
                    cx.entity(),
                    self.focus.clone(),
                    !prose_active,
                ))
        });
        let prose = self.shows_prose_pane().then(|| {
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_prose_mouse_down))
                .on_mouse_move(cx.listener(Self::on_prose_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_prose_mouse_up))
                .on_scroll_wheel(cx.listener(Self::on_prose_scroll))
                .child(self.prose_pane(prose_active, cx))
        });
        let split = source.is_some() && prose.is_some();
        let divider = split.then(|| div().w(px(1.)).h_full().flex_none().bg(colors.border));
        div()
            .track_focus(&self.focus)
            .key_context(if markdown {
                "Editor Markdown"
            } else {
                "Editor"
            })
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
            .on_action(cx.listener(|this, _: &ProseMode, _, cx| {
                this.set_markdown_mode(xenon_settings::MarkdownMode::Prose, cx);
            }))
            .on_action(cx.listener(|this, _: &SplitMode, _, cx| {
                this.set_markdown_mode(xenon_settings::MarkdownMode::Split, cx);
            }))
            .on_action(cx.listener(|this, _: &SourceMode, _, cx| {
                this.set_markdown_mode(xenon_settings::MarkdownMode::Source, cx);
            }))
            .on_action(cx.listener(|this, _: &TogglePaper, _, cx| this.toggle_paper(cx)))
            .on_action(cx.listener(|this, _: &ToggleFullScreen, _, cx| this.toggle_full_screen(cx)))
            .on_action(cx.listener(|this, _: &ToggleTask, _, cx| this.toggle_task(cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(self.page_background(cx))
            .children(alert)
            .children(
                self.worklist_document
                    .then(|| self.render_worklist_raw_header(&colors, cx)),
            )
            .children(lsp_status)
            .children(find_bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .children(source)
                    .children(divider)
                    .children(prose),
            )
            .children(mode_bar)
            .children(strip)
            .children(menu)
            .into_any_element()
    }
}
