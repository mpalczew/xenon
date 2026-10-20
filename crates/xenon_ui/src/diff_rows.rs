//! Draws a parsed diff (see `diff_model`): hunks, split and unified rows.
//! Layout and pairing were decided by the model; this only paints.

use gpui::{
    AnyElement, App, Div, HighlightStyle, Hsla, IntoElement, ParentElement, Pixels, SharedString,
    Styled, StyledText, div, prelude::FluentBuilder, px,
};
use theme::{ActiveTheme, ThemeColors};

use crate::diff_model::{DiffLine, DiffSection, Hunk, Layout, LineKind};

const GUTTER_W: f32 = 44.;
const SIGN_W: f32 = 16.;

/// Colors and type metrics shared by every row of one render.
pub(crate) struct Paint {
    pub colors: ThemeColors,
    pub line_h: Pixels,
    pub font_size: Pixels,
    pub family: SharedString,
    added: Hsla,
    removed: Hsla,
}

impl Paint {
    pub(crate) fn new(cx: &App) -> Self {
        let colors = cx.theme().colors().clone();
        let font = xenon_settings::editor_font(cx);
        let syntax = cx.theme().syntax();
        let sign_color = |name: &str, fallback: Hsla| {
            syntax
                .style_for_name(name)
                .and_then(|style| style.color)
                .unwrap_or(fallback)
        };
        Self {
            added: sign_color("diff.plus", colors.version_control_added),
            removed: sign_color("diff.minus", colors.version_control_deleted),
            line_h: px(font.size * 1.5),
            font_size: px(font.size),
            family: font.family.clone().into(),
            colors,
        }
    }

    fn tint(&self, kind: LineKind) -> Option<Hsla> {
        match kind {
            LineKind::Added => Some(self.added.opacity(0.14)),
            LineKind::Removed => Some(self.removed.opacity(0.14)),
            LineKind::Context => None,
        }
    }
}

/// One section: optional title row, column labels (split), then its hunks.
pub(crate) fn section(
    section: &DiffSection,
    layout: Layout,
    paint: &Paint,
    cx: &App,
) -> AnyElement {
    let colors = &paint.colors;
    let title = section.title.as_ref().map(|(name, detail)| {
        div()
            .flex()
            .items_baseline()
            .gap_3()
            .px_4()
            .pt_4()
            .pb_1()
            .child(
                div()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.text)
                    .child(name.to_uppercase()),
            )
            .child(div().text_color(colors.text_muted).child(detail.clone()))
            .child(counts(section.added, section.removed, paint))
    });
    let labels = (layout == Layout::Split).then(|| {
        div()
            .flex()
            .px_4()
            .pt_2()
            .text_color(colors.text_muted)
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(
                div()
                    .flex_1()
                    .pl(px(GUTTER_W + SIGN_W))
                    .child(section.left_label.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .pl(px(GUTTER_W + SIGN_W))
                    .child(section.right_label.clone()),
            )
    });
    div()
        .flex()
        .flex_col()
        .children(title)
        .children(labels)
        .children(
            section
                .hunks
                .iter()
                .map(|hunk| hunk_block(hunk, layout, paint, cx)),
        )
        .into_any_element()
}

/// `+N -M` totals in the diff and section colors.
pub(crate) fn counts(added: u32, removed: u32, paint: &Paint) -> Div {
    div()
        .flex()
        .gap_2()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(div().text_color(paint.added).child(format!("+{added}")))
        .child(div().text_color(paint.removed).child(format!("-{removed}")))
}

fn hunk_block(hunk: &Hunk, layout: Layout, paint: &Paint, cx: &App) -> Div {
    let colors = &paint.colors;
    let rows: Vec<AnyElement> = match layout {
        Layout::Split => hunk
            .rows
            .iter()
            .map(|row| {
                let left = row.left.map(|i| &hunk.lines[i]);
                let right = row.right.map(|i| &hunk.lines[i]);
                div()
                    .flex()
                    .w_full()
                    .child(split_cell(left, true, paint, cx))
                    .child(
                        split_cell(right, false, paint, cx)
                            .border_l_1()
                            .border_color(colors.border),
                    )
                    .into_any_element()
            })
            .collect(),
        Layout::Unified => hunk
            .lines
            .iter()
            .map(|line| unified_row(line, paint, cx))
            .collect(),
    };
    div()
        .mx_4()
        .mt_3()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .overflow_hidden()
        .child(
            div()
                .px_3()
                .py_0p5()
                .bg(colors.element_background)
                .text_color(colors.text_accent)
                .child(hunk.header.clone()),
        )
        .children(rows)
}

fn split_cell(line: Option<&DiffLine>, left: bool, paint: &Paint, cx: &App) -> Div {
    let colors = &paint.colors;
    let Some(line) = line else {
        return div()
            .flex_1()
            .min_w_0()
            .bg(colors.text_muted.opacity(0.05))
            .min_h(paint.line_h);
    };
    let number = if left { line.old_no } else { line.new_no };
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .min_h(paint.line_h)
        .when_some(paint.tint(line.kind), |cell, tint| cell.bg(tint))
        .child(gutter(number, paint))
        .child(sign(line.kind, paint))
        .child(code(line, paint, cx))
}

fn unified_row(line: &DiffLine, paint: &Paint, cx: &App) -> AnyElement {
    div()
        .flex()
        .w_full()
        .min_h(paint.line_h)
        .when_some(paint.tint(line.kind), |row, tint| row.bg(tint))
        .child(gutter(line.old_no, paint))
        .child(gutter(line.new_no, paint))
        .child(sign(line.kind, paint))
        .child(code(line, paint, cx))
        .into_any_element()
}

fn gutter(number: Option<u32>, paint: &Paint) -> Div {
    div()
        .w(px(GUTTER_W))
        .flex_none()
        .flex()
        .justify_end()
        .pr_2()
        .text_color(paint.colors.text_muted)
        .children(number.map(|n| n.to_string()))
}

fn sign(kind: LineKind, paint: &Paint) -> Div {
    let (glyph, color) = match kind {
        LineKind::Added => ("+", paint.added),
        LineKind::Removed => ("-", paint.removed),
        LineKind::Context => ("", paint.colors.text_muted),
    };
    div()
        .w(px(SIGN_W))
        .flex_none()
        .flex()
        .justify_center()
        .text_color(color)
        .child(glyph)
}

/// The line's text with syntax colors and changed-word backgrounds.
fn code(line: &DiffLine, paint: &Paint, cx: &App) -> Div {
    let colors = &paint.colors;
    let syntax = cx.theme().syntax();
    let word = match line.kind {
        LineKind::Added => colors.version_control_word_added,
        _ => colors.version_control_word_deleted,
    };
    let mut text = String::new();
    let mut highlights = Vec::new();
    for seg in &line.segs {
        let start = text.len();
        text.push_str(&seg.text);
        let style = seg.syntax.and_then(|name| syntax.style_for_name(name));
        let highlight = HighlightStyle {
            color: style.and_then(|s| s.color),
            font_style: style.and_then(|s| s.font_style),
            background_color: seg.emphasis.then_some(word),
            ..Default::default()
        };
        if highlight != HighlightStyle::default() {
            highlights.push((start..text.len(), highlight));
        }
    }
    div()
        .flex_1()
        .min_w_0()
        .pr_2()
        .child(StyledText::new(text).with_highlights(highlights))
}

/// Whole-diff font setup for the body column.
pub(crate) fn body_text(base: Div, paint: &Paint) -> Div {
    base.font_family(paint.family.clone())
        .text_size(paint.font_size)
        .line_height(paint.line_h)
        .text_color(paint.colors.editor_foreground)
}
