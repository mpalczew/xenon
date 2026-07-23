//! Markdown table block assembly for the preview builder.

use std::ops::Range;

use gpui::{
    AnyElement, Entity, FontWeight, HighlightStyle, Hsla, IntoElement, ParentElement, Pixels,
    SharedString, Styled, div,
};
use pulldown_cmark::Alignment;

use super::selectable::SelectableBlock;
use super::state::PreviewState;

#[derive(Default)]
pub(super) struct Table {
    pub alignments: Vec<Alignment>,
    pub rows: Vec<TableRow>,
    pub current: TableRow,
    pub in_head: bool,
    pub in_cell: bool,
}

impl Table {
    pub(super) fn finish_cell(
        &mut self,
        text: String,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
        source_range: Range<usize>,
    ) {
        self.in_cell = false;
        self.current.cells.push(TableCell {
            text: SharedString::from(text),
            highlights,
            header: self.in_head,
            source_range,
        });
    }

    pub(super) fn finish_row(&mut self) {
        let row = std::mem::take(&mut self.current);
        self.rows.push(row);
    }
}

#[derive(Default)]
pub(super) struct TableRow {
    pub cells: Vec<TableCell>,
}

pub(super) struct TableCell {
    pub text: SharedString,
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
    pub header: bool,
    pub source_range: Range<usize>,
}

/// Shared paint tokens for table chrome + selectable cells.
pub(super) struct TablePaint {
    pub rule: Hsla,
    pub code_bg: Hsla,
    pub base: Pixels,
    pub host: Entity<PreviewState>,
    pub selection: Hsla,
}

/// Render table; return plain texts + source ranges in document order.
pub(super) fn render_table(
    table: Table,
    paint: &TablePaint,
    block_base: usize,
) -> (AnyElement, Vec<SharedString>, Vec<Range<usize>>) {
    let mut plains = Vec::new();
    let mut ranges = Vec::new();
    let mut next_ix = block_base;
    let mut rows = div()
        .my_3()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .border_1()
        .border_color(paint.rule);
    for row in table.rows {
        let (row_el, row_plains, row_ranges) = table_row(row, &table.alignments, paint, next_ix);
        next_ix += row_plains.len();
        plains.extend(row_plains);
        ranges.extend(row_ranges);
        rows = rows.child(row_el);
    }
    (rows.into_any_element(), plains, ranges)
}

fn table_row(
    row: TableRow,
    alignments: &[Alignment],
    paint: &TablePaint,
    mut block_ix: usize,
) -> (AnyElement, Vec<SharedString>, Vec<Range<usize>>) {
    let mut plains = Vec::new();
    let mut ranges = Vec::new();
    let mut element = div()
        .w_full()
        .min_w_0()
        .flex()
        .border_b_1()
        .border_color(paint.rule);
    for (index, cell) in row.cells.into_iter().enumerate() {
        let text = cell.text.clone();
        plains.push(text.clone());
        ranges.push(cell.source_range.clone());
        let child = SelectableBlock::new(
            block_ix,
            text,
            cell.highlights,
            paint.host.clone(),
            paint.selection,
        );
        block_ix += 1;
        element = element.child(table_cell_shell(
            child.into_any_element(),
            cell.header,
            alignments.get(index).copied(),
            paint,
        ));
    }
    (element.into_any_element(), plains, ranges)
}

fn table_cell_shell(
    child: AnyElement,
    header: bool,
    alignment: Option<Alignment>,
    paint: &TablePaint,
) -> AnyElement {
    let mut element = div()
        .flex_1()
        .min_w_0()
        .px_2()
        .py_1()
        .border_r_1()
        .border_color(paint.rule)
        .whitespace_normal()
        .text_size(paint.base);
    if header {
        element = element.bg(paint.code_bg).font_weight(FontWeight::BOLD);
    }
    element = match alignment.unwrap_or(Alignment::None) {
        Alignment::Center => element.text_center(),
        Alignment::Right => element.text_right(),
        Alignment::None | Alignment::Left => element.text_left(),
    };
    element.child(child).into_any_element()
}
