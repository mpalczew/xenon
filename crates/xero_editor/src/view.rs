//! `EditorView`: a focusable gpui view over a `Buffer`. Text input flows through
//! an `EntityInputHandler`; editing/navigation keys go through key-down; Cmd-S
//! saves. Mirrors the terminal view's input wiring.

use std::ops::Range;
use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    App, AppContext, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Point, Render,
    ScrollWheelEvent, Styled, UTF16Selection, Window, canvas, div, px,
};
use theme::ActiveTheme;

use crate::buffer::Buffer;
use crate::edit::{EditCommand, Motion};
use crate::element::{self, ColoredSpan};
use crate::highlight;

const LINE_HEIGHT_MULTIPLIER: f32 = 1.3;

pub struct EditorView {
    buffer: Buffer,
    highlights: Vec<highlight::Span>,
    scroll_top: Pixels,
    focus: FocusHandle,
    autofocus: bool,
    focused_once: bool,
    /// Render the markdown preview instead of the source (markdown files only).
    preview: bool,
}

impl EditorView {
    /// Open `path` and wrap it in an entity. `autofocus` grabs keyboard focus on
    /// first render (true for user-opened files, false for agent-opened ones so
    /// the terminal keeps focus).
    pub fn build(path: PathBuf, autofocus: bool, cx: &mut App) -> Result<Entity<Self>> {
        let buffer = Buffer::open(&path)?;
        Ok(cx.new(|cx| Self::from_buffer(buffer, autofocus, cx)))
    }

    fn from_buffer(buffer: Buffer, autofocus: bool, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            buffer,
            highlights: Vec::new(),
            scroll_top: px(0.),
            focus: cx.focus_handle(),
            autofocus,
            focused_once: false,
            preview: false,
        };
        view.recompute_highlights();
        view
    }

    /// The file this editor is showing.
    pub fn path(&self) -> &std::path::Path {
        self.buffer.path()
    }

    /// Whether this file can show a markdown preview.
    pub fn is_markdown(&self) -> bool {
        matches!(
            self.buffer.path().extension().and_then(|e| e.to_str()),
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

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let line_height = element::line_height(px(xero_settings::font_size(cx)), LINE_HEIGHT_MULTIPLIER);
        let delta = event.delta.pixel_delta(line_height).y;
        // Lower-bound only; the upper bound needs the viewport height, so `layout`
        // clamps it against the content each frame.
        self.scroll_top = (self.scroll_top - delta).max(px(0.));
        cx.notify();
    }

    /// Re-highlight the whole buffer. Cheap enough for v1 file sizes; called
    /// after every edit. Unsupported languages get no spans (default color).
    fn recompute_highlights(&mut self) {
        self.highlights = highlight::spans_for_path(self.buffer.path(), &self.buffer.text());
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.platform && keystroke.key == "s" {
            if let Err(error) = self.buffer.save() {
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
        self.buffer.apply(command);
        if edits {
            self.recompute_highlights();
        }
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
        if self.preview {
            let size = px(xero_settings::font_size(cx));
            return div()
                .track_focus(&self.focus)
                .key_context("Editor")
                .on_key_down(cx.listener(Self::on_key))
                .size_full()
                .bg(colors.editor_background)
                .child(crate::markdown::render(&self.buffer.text(), size, cx))
                .into_any_element();
        }
        div()
            .track_focus(&self.focus)
            .key_context("Editor")
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .size_full()
            .bg(colors.editor_background)
            .child(editor_canvas(cx.entity(), self.focus.clone()))
            .into_any_element()
    }
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
    // Clamp the scroll so the last line rests at the viewport bottom, not the
    // top. Needs the viewport height, which is only known here (in layout).
    let content_height = line_height * (view.read(cx).buffer.rope().len_lines() as f32);
    let max_scroll = (content_height - bounds.size.height).max(px(0.));
    view.update(cx, |v, _| v.scroll_top = v.scroll_top.min(max_scroll));
    let view = view.read(cx);
    let theme = cx.theme();
    let text_color = theme.colors().editor_foreground;
    let syntax = theme.syntax();
    // Resolve each highlight span to a concrete color against the active theme.
    let spans: Vec<ColoredSpan> = view
        .highlights
        .iter()
        .map(|span| {
            let color = syntax
                .style_for_name(span.name)
                .and_then(|style| style.color)
                .unwrap_or(text_color);
            ColoredSpan { start: span.start, end: span.end, color }
        })
        .collect();
    element::layout(
        view.buffer.rope(),
        view.buffer.cursor_position(),
        text_color,
        &spans,
        bounds.origin,
        bounds.size.height,
        view.scroll_top,
        font,
        size,
        line_height,
        window,
    )
}

impl EntityInputHandler for EditorView {
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !text.is_empty() {
            self.buffer.apply(EditCommand::Insert(text.to_string()));
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
        if !new_text.is_empty() {
            self.buffer.apply(EditCommand::Insert(new_text.to_string()));
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
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
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
