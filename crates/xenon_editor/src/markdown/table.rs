//! Markdown table block assembly for the preview builder.

use std::ops::Range;

use gpui::{
    AnyElement, FontWeight, HighlightStyle, Hsla, IntoElement, ParentElement, Pixels, SharedString,
    Styled, StyledText, div,
};
use pulldown_cmark::Alignment;

#[derive(Default)]
pub(super) struct Table {
    pub alignments: Vec<Alignment>,
    pub rows: Vec<TableRow>,
    pub current: TableRow,
    pub in_head: bool,
    pub in_cell: bool,
}

#[derive(Default)]
pub(super) struct TableRow {
    pub cells: Vec<TableCell>,
}

pub(super) struct TableCell {
    pub text: SharedString,
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
    pub header: bool,
}

pub(super) fn render_table(table: Table, rule: Hsla, code_bg: Hsla, base: Pixels) -> AnyElement {
    let mut rows = div()
        .my_3()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .border_1()
        .border_color(rule);
    for row in table.rows {
        rows = rows.child(table_row(row, &table.alignments, rule, code_bg, base));
    }
    rows.into_any_element()
}

fn table_row(
    row: TableRow,
    alignments: &[Alignment],
    rule: Hsla,
    code_bg: Hsla,
    base: Pixels,
) -> AnyElement {
    let mut element = div()
        .w_full()
        .min_w_0()
        .flex()
        .border_b_1()
        .border_color(rule);
    for (index, cell) in row.cells.into_iter().enumerate() {
        element = element.child(table_cell(
            cell,
            alignments.get(index).copied(),
            rule,
            code_bg,
            base,
        ));
    }
    element.into_any_element()
}

fn table_cell(
    cell: TableCell,
    alignment: Option<Alignment>,
    rule: Hsla,
    code_bg: Hsla,
    base: Pixels,
) -> AnyElement {
    let mut element = div()
        .flex_1()
        .min_w_0()
        .px_2()
        .py_1()
        .border_r_1()
        .border_color(rule)
        .whitespace_normal()
        .text_size(base);
    if cell.header {
        element = element.bg(code_bg).font_weight(FontWeight::BOLD);
    }
    element = match alignment.unwrap_or(Alignment::None) {
        Alignment::Center => element.text_center(),
        Alignment::Right => element.text_right(),
        Alignment::None | Alignment::Left => element.text_left(),
    };
    element
        .child(StyledText::new(cell.text).with_highlights(cell.highlights))
        .into_any_element()
}
