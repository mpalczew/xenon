//! Editor canvas layout: theme spans, scroll follow, click hit-test cache.

use crate::markdown::{PreviewRender, PreviewState};

/// Markdown preview using the current editor font settings.
pub(crate) fn markdown_preview(
    source: &str,
    host: gpui::Entity<PreviewState>,
    cx: &gpui::App,
) -> PreviewRender {
    let face = xenon_settings::editor_font(cx);
    crate::markdown::render(source, gpui::px(face.size), &face.family, host, cx)
}

impl super::EditorView {
    pub fn is_dirty(&self) -> bool {
        match &self.content {
            super::Content::Text(buffer) => buffer.is_dirty(),
            _ => false,
        }
    }
}

use gpui::{
    App, Bounds, ElementInputHandler, Entity, FocusHandle, Pixels, Styled, Window, canvas, px,
};
use theme::ActiveTheme;

use super::{Content, EditorView, LINE_HEIGHT_MULTIPLIER};
use crate::element::{self, ColoredSpan, DiagnosticRange};
use crate::mouse::ClickLayout;

pub(super) fn editor_canvas(
    view: Entity<EditorView>,
    focus: FocusHandle,
) -> impl gpui::IntoElement {
    canvas(
        {
            let view = view.clone();
            move |bounds, window, cx| layout(&view, bounds, window, cx)
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
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> element::EditorLayout {
    let face = xenon_settings::editor_font(cx);
    let size = px(face.size);
    let line_height = element::line_height(size, LINE_HEIGHT_MULTIPLIER);
    let lines = match &view.read(cx).content {
        Content::Text(buffer) => buffer.rope().len_lines(),
        Content::Image(_) | Content::Unsupported { .. } => 0,
    };
    let content_height = line_height * (lines as f32);
    let max_scroll = (content_height - bounds.size.height).max(px(0.));
    let follow_cursor = follow_cursor(view, max_scroll, cx);

    let show_line_numbers = xenon_settings::show_line_numbers(cx);
    let editor_layout = {
        let view = view.read(cx);
        let theme = cx.theme();
        let text_color = theme.colors().editor_foreground;
        let syntax = theme.syntax();
        let spans: Vec<ColoredSpan> = view
            .highlights
            .iter()
            .map(|span| {
                let style = syntax.style_for_name(span.name);
                let color = style.and_then(|s| s.color).unwrap_or(text_color);
                ColoredSpan {
                    start: span.start,
                    end: span.end,
                    color,
                    font_weight: style.and_then(|s| s.font_weight),
                    font_style: style.and_then(|s| s.font_style),
                }
            })
            .collect();
        let Content::Text(buffer) = &view.content else {
            unreachable!("non-text editor content does not render the text canvas");
        };
        let (search_matches, search_current) = match &view.find {
            Some(find) if find.open && !find.matches.is_empty() => {
                let current = find.current.and_then(|i| find.matches.get(i).cloned());
                (find.matches.as_slice(), current)
            }
            _ => (&[][..], None),
        };
        let diagnostics = diagnostic_ranges(view, theme);
        element::layout(
            element::LayoutInput {
                rope: buffer.rope(),
                cursor: buffer.cursor_position(),
                selection: buffer.selection_range(),
                selection_color: theme.colors().element_selected,
                search_matches,
                search_current,
                search_match_color: theme.colors().search_match_background,
                search_current_color: theme.colors().search_active_match_background,
                occurrences: &view.occurrence_ranges,
                occurrence_color: theme.colors().element_hover,
                diagnostics: &diagnostics,
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
                follow_cursor,
            },
            element::TextMetrics {
                font: &element::editor_font(&face.family),
                font_size: size,
                line_height,
            },
            window,
        )
    };
    view.update(cx, |view, _| {
        view.scroll_top = editor_layout.scroll_top;
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

fn follow_cursor(view: &Entity<EditorView>, max_scroll: Pixels, cx: &mut App) -> bool {
    view.update(cx, |view, _| {
        let follow = match &view.content {
            Content::Text(buffer) => {
                let cursor = buffer.cursor_position();
                let moved = view.last_cursor != Some(cursor);
                if moved {
                    view.last_cursor = Some(cursor);
                }
                moved
            }
            Content::Image(_) | Content::Unsupported { .. } => false,
        };
        view.scroll_top = view.scroll_top.min(max_scroll);
        follow
    })
}

fn diagnostic_ranges(view: &EditorView, theme: &theme::Theme) -> Vec<DiagnosticRange> {
    let status = theme.status();
    view.diagnostics
        .iter()
        .map(|diagnostic| DiagnosticRange {
            range: diagnostic.range.clone(),
            color: match diagnostic.severity {
                super::EditorDiagnosticSeverity::Error => status.error,
                super::EditorDiagnosticSeverity::Warning => status.warning,
            },
        })
        .collect()
}
