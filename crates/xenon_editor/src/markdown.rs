//! Render a markdown document to gpui elements for the preview pane. Block
//! structure comes from `pulldown-cmark`; inline emphasis/links/code use
//! `StyledText` highlights; fenced code blocks are syntax-highlighted via the
//! tree-sitter highlighters, keyed by the fence language.

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, StrikethroughStyle, Styled,
    StyledText, div, px,
};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use theme::{ActiveTheme, Theme};

use crate::highlight;

const TABLE_CELL_WIDTH: f32 = 160.;

/// Render markdown `source` into a scrollable column of block elements sized
/// relative to `base`, using `mono_family` for code spans and fences.
pub fn render(source: &str, base: Pixels, mono_family: &str, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let colors = theme.colors().clone();
    let mut builder = Builder {
        theme,
        accent: colors.text_accent,
        code_bg: colors.surface_background,
        rule: colors.border,
        base,
        mono: mono_family.to_string(),
        blocks: Vec::new(),
        inline: String::new(),
        highlights: Vec::new(),
        bold: 0,
        italic: 0,
        strike: 0,
        link: 0,
        code: None,
        table: None,
    };
    for event in Parser::new_ext(source, markdown_options()) {
        builder.event(event);
    }
    preview_surface(builder.blocks, base, colors.text, colors.editor_background)
}

struct Builder {
    theme: Arc<Theme>,
    accent: Hsla,
    code_bg: Hsla,
    rule: Hsla,
    base: Pixels,
    mono: String,
    blocks: Vec<AnyElement>,
    // Current inline run being accumulated for the open block.
    inline: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    bold: u32,
    italic: u32,
    strike: u32,
    link: u32,
    // When inside a fenced block: (language, collected source).
    code: Option<(String, String)>,
    table: Option<Table>,
}

struct Table {
    alignments: Vec<Alignment>,
    rows: Vec<TableRow>,
    current: TableRow,
    in_head: bool,
    in_cell: bool,
}

#[derive(Default)]
struct TableRow {
    cells: Vec<TableCell>,
}

struct TableCell {
    text: SharedString,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    header: bool,
}

impl Builder {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => match &mut self.code {
                Some((_, buffer)) => buffer.push_str(&text),
                None => {
                    let style = self.inline_style();
                    self.push(&text, style);
                }
            },
            Event::Code(text) => {
                let style = HighlightStyle {
                    color: Some(self.accent),
                    background_color: Some(self.code_bg),
                    ..Default::default()
                };
                self.push(&text, Some(style));
            }
            Event::FootnoteReference(label) => {
                let style = HighlightStyle {
                    color: Some(self.accent),
                    ..Default::default()
                };
                self.push(&format!("[^{label}]"), Some(style));
            }
            Event::TaskListMarker(checked) => {
                self.inline.push_str(if checked { "[x] " } else { "[ ] " });
            }
            Event::SoftBreak if self.code.is_none() => self.inline.push(' '),
            Event::HardBreak if self.code.is_none() => self.inline.push('\n'),
            Event::Rule => self
                .blocks
                .push(div().my_3().h(px(1.)).bg(self.rule).into_any_element()),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { .. } => {}
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { .. } => self.link += 1,
            Tag::FootnoteDefinition(label) => {
                self.inline.push_str(&format!("[^{label}]: "));
            }
            Tag::Item => self.inline.push_str("• "),
            Tag::Table(alignments) => {
                self.table = Some(Table {
                    alignments,
                    rows: Vec::new(),
                    current: TableRow::default(),
                    in_head: false,
                    in_cell: false,
                });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = true;
                }
            }
            Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.current = TableRow::default();
                }
            }
            Tag::TableCell => {
                if let Some(table) = &mut self.table {
                    table.in_cell = true;
                }
                self.inline.clear();
                self.highlights.clear();
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph if !self.in_table_cell() => self.flush_block(self.base, false, None),
            TagEnd::Paragraph => {}
            TagEnd::Heading(level) => self.flush_block(heading_size(self.base, level), true, None),
            TagEnd::Item => self.flush_block(self.base, false, Some(px(16.))),
            TagEnd::CodeBlock => self.flush_code(),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => self.link = self.link.saturating_sub(1),
            TagEnd::FootnoteDefinition => {
                self.flush_block(px(f32::from(self.base) * 0.9), false, None)
            }
            TagEnd::TableCell => self.finish_table_cell(),
            TagEnd::TableRow => self.finish_table_row(),
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = false;
                }
            }
            TagEnd::Table => self.flush_table(),
            _ => {}
        }
    }

    /// The inline style implied by the currently open emphasis/strong/link tags.
    fn inline_style(&self) -> Option<HighlightStyle> {
        if self.bold == 0 && self.italic == 0 && self.strike == 0 && self.link == 0 {
            return None;
        }
        Some(HighlightStyle {
            font_weight: (self.bold > 0).then_some(FontWeight::BOLD),
            font_style: (self.italic > 0).then_some(FontStyle::Italic),
            color: (self.link > 0).then_some(self.accent),
            strikethrough: (self.strike > 0).then_some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
            }),
            ..Default::default()
        })
    }

    fn push(&mut self, text: &str, style: Option<HighlightStyle>) {
        let start = self.inline.len();
        self.inline.push_str(text);
        if let Some(style) = style {
            self.highlights.push((start..self.inline.len(), style));
        }
    }

    /// Emit the accumulated inline run as one text block, then reset it.
    fn flush_block(&mut self, size: Pixels, bold: bool, pad_left: Option<Pixels>) {
        if self.inline.trim().is_empty() {
            self.inline.clear();
            self.highlights.clear();
            return;
        }
        let text = SharedString::from(std::mem::take(&mut self.inline));
        let highlights = std::mem::take(&mut self.highlights);
        let mut block = div().my_1().text_size(size);
        if bold {
            block = block.mt_3().font_weight(FontWeight::BOLD);
        }
        if let Some(pad) = pad_left {
            block = block.pl(pad);
        }
        self.blocks.push(
            block
                .child(StyledText::new(text).with_highlights(highlights))
                .into_any_element(),
        );
    }

    fn flush_code(&mut self) {
        let Some((lang, mut code)) = self.code.take() else {
            return;
        };
        if code.ends_with('\n') {
            code.pop();
        }
        let width = code_block_width(&code, self.base);
        let highlights: Vec<_> = highlight::spans_for_lang(&lang, &code)
            .into_iter()
            .map(|span| {
                let style = HighlightStyle {
                    color: self.code_color(span.name),
                    ..Default::default()
                };
                (span.start..span.end, style)
            })
            .collect();
        self.blocks.push(
            wide_block(width)
                .my_2()
                .p_2()
                .rounded_md()
                .bg(self.code_bg)
                .font_family(self.mono.clone())
                .text_size(self.base)
                .child(StyledText::new(SharedString::from(code)).with_highlights(highlights))
                .into_any_element(),
        );
    }

    fn in_table_cell(&self) -> bool {
        self.table.as_ref().is_some_and(|table| table.in_cell)
    }

    fn finish_table_cell(&mut self) {
        let Some(table) = &mut self.table else {
            return;
        };
        table.in_cell = false;
        table.current.cells.push(TableCell {
            text: SharedString::from(std::mem::take(&mut self.inline)),
            highlights: std::mem::take(&mut self.highlights),
            header: table.in_head,
        });
    }

    fn finish_table_row(&mut self) {
        let Some(table) = &mut self.table else {
            return;
        };
        let row = std::mem::take(&mut table.current);
        table.rows.push(row);
    }

    fn flush_table(&mut self) {
        let Some(table) = self.table.take() else {
            return;
        };
        let width = table_width(&table);
        let mut rows = wide_block(width)
            .my_3()
            .flex()
            .flex_col()
            .border_1()
            .border_color(self.rule);
        for row in table.rows {
            rows = rows.child(self.table_row(row, &table.alignments));
        }
        self.blocks.push(rows.into_any_element());
    }

    fn table_row(&self, row: TableRow, alignments: &[Alignment]) -> AnyElement {
        let mut element = div().flex().border_b_1().border_color(self.rule);
        for (index, cell) in row.cells.into_iter().enumerate() {
            element = element.child(self.table_cell(cell, alignments.get(index).copied()));
        }
        element.into_any_element()
    }

    fn table_cell(&self, cell: TableCell, alignment: Option<Alignment>) -> AnyElement {
        let mut element = div()
            .flex_1()
            .px_2()
            .py_1()
            .border_r_1()
            .border_color(self.rule)
            .text_size(self.base);
        if cell.header {
            element = element.bg(self.code_bg).font_weight(FontWeight::BOLD);
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

    fn code_color(&self, name: &str) -> Option<Hsla> {
        self.theme
            .syntax()
            .style_for_name(name)
            .and_then(|style| style.color)
    }
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
}

fn preview_surface(
    blocks: Vec<AnyElement>,
    base: Pixels,
    text: Hsla,
    background: Hsla,
) -> AnyElement {
    div()
        .id("md-preview")
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_x_scroll()
        .overflow_y_scroll()
        .p_4()
        .text_size(base)
        .text_color(text)
        .bg(background)
        .children(blocks)
        .into_any_element()
}

fn wide_block(width: Pixels) -> gpui::Div {
    div().w(width)
}

fn code_block_width(code: &str, base: Pixels) -> Pixels {
    let cols = code.lines().map(str::len).max().unwrap_or(1).max(32);
    px(cols as f32 * f32::from(base) * 0.62 + 32.)
}

fn table_width(table: &Table) -> Pixels {
    let cols = table
        .rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(table.alignments.len())
        .max(table.alignments.len())
        .max(1);
    px(cols as f32 * TABLE_CELL_WIDTH)
}

fn heading_size(base: Pixels, level: HeadingLevel) -> Pixels {
    let scale = match level {
        HeadingLevel::H1 => 1.7,
        HeadingLevel::H2 => 1.45,
        HeadingLevel::H3 => 1.25,
        HeadingLevel::H4 => 1.15,
        _ => 1.05,
    };
    px(f32::from(base) * scale)
}
