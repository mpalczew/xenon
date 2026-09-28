//! Event-driven markdown → gpui block builder (pulldown-cmark stream).

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, Entity, FontStyle, FontWeight, HighlightStyle, Hsla, IntoElement, ParentElement,
    Pixels, SharedString, StrikethroughStyle, Styled, div, px,
};
use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use theme::Theme;

use super::nav::OutlineEntry;
mod definition;

use super::selectable::SelectableBlock;
use super::state::PreviewState;
use super::table::{Table, TablePaint, render_table};
use super::yaml::{MetaPaint, render_metadata};
use super::{code_block_width, heading_size, prose_slot, wide_block, wide_slot};
use crate::highlight;

pub(super) struct Builder {
    theme: Arc<Theme>,
    accent: Hsla,
    muted: Hsla,
    code_bg: Hsla,
    rule: Hsla,
    selection_color: Hsla,
    base: Pixels,
    mono: String,
    host: Entity<PreviewState>,
    blocks: Vec<AnyElement>,
    plain: Vec<SharedString>,
    /// Parallel to `plain`: byte range in the original markdown source.
    source_ranges: Vec<Range<usize>>,
    inline: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    bold: u32,
    italic: u32,
    strike: u32,
    link: u32,
    code: Option<(String, String)>,
    metadata: Option<String>,
    table: Option<Table>,
    def_title: Option<String>,
    def_title_source: Option<Range<usize>>,
    /// Source range for the in-progress table cell (from Start/End TableCell).
    cell_source: Option<Range<usize>>,
    /// Open lists: `None` = unordered; `Some(n)` = next ordered number.
    list_stack: Vec<Option<u64>>,
    /// Source ranges for open list items (stack; nested items push).
    item_sources: Vec<Range<usize>>,
    outline: Vec<OutlineEntry>,
}

/// Finished preview: blocks plus the parallel selection maps and the outline.
pub(super) struct Built {
    pub blocks: Vec<AnyElement>,
    pub plain: Vec<SharedString>,
    pub source_ranges: Vec<Range<usize>>,
    pub outline: Vec<OutlineEntry>,
}

pub(super) struct BuilderColors {
    pub accent: Hsla,
    pub muted: Hsla,
    pub code_bg: Hsla,
    pub rule: Hsla,
    pub selection: Hsla,
}

impl Builder {
    pub(super) fn new(
        theme: Arc<Theme>,
        colors: BuilderColors,
        base: Pixels,
        mono_family: &str,
        host: Entity<PreviewState>,
    ) -> Self {
        Self {
            theme,
            accent: colors.accent,
            muted: colors.muted,
            code_bg: colors.code_bg,
            rule: colors.rule,
            selection_color: colors.selection,
            base,
            mono: mono_family.to_string(),
            host,
            blocks: Vec::new(),
            plain: Vec::new(),
            source_ranges: Vec::new(),
            inline: String::new(),
            highlights: Vec::new(),
            bold: 0,
            italic: 0,
            strike: 0,
            link: 0,
            code: None,
            metadata: None,
            table: None,
            def_title: None,
            def_title_source: None,
            cell_source: None,
            list_stack: Vec::new(),
            item_sources: Vec::new(),
            outline: Vec::new(),
        }
    }

    fn list_pad(&self) -> Option<Pixels> {
        (!self.list_stack.is_empty()).then_some(px(20. * self.list_stack.len() as f32))
    }

    fn push_item_marker(&mut self) {
        let marker = match self.list_stack.last_mut() {
            Some(Some(n)) => {
                let s = format!("{n}. ");
                *n += 1;
                s
            }
            _ => "• ".into(),
        };
        self.inline.push_str(&marker);
    }

    /// Tight lists emit item text then a nested `List` with no `Paragraph` end —
    /// flush the lead-in so nested items are separate blocks (not `• a• b`).
    fn push_list(&mut self, start: Option<u64>, list_range: Range<usize>) {
        if !self.inline.trim().is_empty() {
            let src = self
                .item_sources
                .last()
                .map(|item| item.start..list_range.start)
                .unwrap_or_else(|| list_range.clone());
            self.flush_block(self.base, false, self.list_pad(), src);
        }
        self.list_stack.push(start);
    }

    pub(super) fn finish(self) -> Built {
        Built {
            blocks: self.blocks,
            plain: self.plain,
            source_ranges: self.source_ranges,
            outline: self.outline,
        }
    }

    /// Prose-width block centered in the reading column.
    fn push_block(&mut self, element: impl IntoElement) {
        self.blocks.push(prose_slot(element, self.base));
    }
    fn push_selectable(
        &mut self,
        text: SharedString,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
        source_range: Range<usize>,
    ) -> AnyElement {
        let ix = self.plain.len();
        self.plain.push(text.clone());
        self.source_ranges.push(source_range);
        SelectableBlock::new(
            ix,
            text,
            highlights,
            self.host.clone(),
            self.selection_color,
        )
        .into_any_element()
    }
    fn push_meta_nl(&mut self) {
        if let Some(buffer) = &mut self.metadata {
            buffer.push('\n');
        }
    }

    pub(super) fn event(&mut self, event: Event, range: Range<usize>) {
        match event {
            Event::Start(tag) => self.start(tag, range),
            Event::End(tag) => self.end(tag, range),
            Event::Text(text) => {
                if let Some(buffer) = &mut self.metadata {
                    buffer.push_str(&text);
                } else if let Some((_, buffer)) = &mut self.code {
                    buffer.push_str(&text);
                } else {
                    let style = self.inline_style();
                    self.push(&text, style);
                }
            }
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
            Event::SoftBreak if self.metadata.is_some() => self.push_meta_nl(),
            Event::SoftBreak if self.code.is_none() => self.inline.push(' '),
            Event::HardBreak if self.metadata.is_some() => self.push_meta_nl(),
            Event::HardBreak if self.code.is_none() => self.inline.push('\n'),
            Event::Rule => self.push_block(div().my_3().h(px(1.)).bg(self.rule)),
            _ => {}
        }
    }
    fn start(&mut self, tag: Tag, range: Range<usize>) {
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
            Tag::MetadataBlock(_) => {
                self.metadata = Some(String::new());
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { .. } => self.link += 1,
            Tag::FootnoteDefinition(label) => {
                self.inline.push_str(&format!("[^{label}]: "));
            }
            Tag::List(start) => self.push_list(start, range),
            Tag::Item => {
                self.item_sources.push(range);
                self.push_item_marker();
            }
            Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                self.inline.clear();
                self.highlights.clear();
            }
            Tag::Table(alignments) => {
                self.table = Some(Table {
                    alignments,
                    ..Default::default()
                });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = true;
                }
            }
            Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.current = Default::default();
                }
            }
            Tag::TableCell => {
                if let Some(table) = &mut self.table {
                    table.in_cell = true;
                }
                self.cell_source = Some(range);
                self.inline.clear();
                self.highlights.clear();
            }
            _ => {}
        }
    }
    fn end(&mut self, tag: TagEnd, range: Range<usize>) {
        match tag {
            TagEnd::Paragraph if !self.in_table_cell() => {
                self.flush_block(self.base, false, self.list_pad(), range)
            }
            TagEnd::Paragraph => {}
            TagEnd::Heading(level) => {
                let title = SharedString::from(self.inline.trim().to_string());
                let block = self.blocks.len();
                self.flush_block(heading_size(self.base, level), true, None, range);
                if self.blocks.len() > block {
                    self.outline.push(OutlineEntry {
                        level: level as u8,
                        title,
                        block,
                    });
                }
            }
            TagEnd::Item => {
                let src = self.item_sources.pop().unwrap_or(range);
                self.flush_block(self.base, false, self.list_pad(), src);
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
            }
            TagEnd::CodeBlock => self.flush_code(range),
            TagEnd::MetadataBlock(_) => self.flush_metadata(range),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => self.link = self.link.saturating_sub(1),
            TagEnd::FootnoteDefinition => {
                self.flush_block(px(f32::from(self.base) * 0.9), false, None, range)
            }
            TagEnd::DefinitionListTitle => {
                self.def_title = Some(std::mem::take(&mut self.inline).trim().to_string());
                self.def_title_source = Some(range);
                self.highlights.clear();
            }
            TagEnd::DefinitionListDefinition => self.flush_definition(range),
            TagEnd::TableCell => self.finish_table_cell(range),
            TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    table.finish_row();
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = false;
                    // pulldown-cmark emits the header cells directly inside
                    // TableHead; only body rows get a TableRow wrapper.
                    table.finish_row();
                }
            }
            TagEnd::Table => self.flush_table(),
            _ => {}
        }
    }
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
    fn flush_block(
        &mut self,
        size: Pixels,
        bold: bool,
        pad_left: Option<Pixels>,
        source_range: Range<usize>,
    ) {
        if self.inline.trim().is_empty() {
            self.inline.clear();
            self.highlights.clear();
            return;
        }
        let text = SharedString::from(std::mem::take(&mut self.inline));
        let highlights = std::mem::take(&mut self.highlights);
        let child = self.push_selectable(text, highlights, source_range);
        let mut block = div()
            .my_1()
            .w_full()
            .min_w_0()
            .whitespace_normal()
            .text_size(size);
        if bold {
            block = block.mt_3().font_weight(FontWeight::BOLD);
        }
        if let Some(pad) = pad_left {
            block = block.pl(pad);
        }
        self.push_block(block.child(child));
    }
    fn flush_code(&mut self, source_range: Range<usize>) {
        let Some((lang, mut code)) = self.code.take() else {
            return;
        };
        if code.ends_with('\n') {
            code.pop();
        }
        let content = code_block_width(&code, self.base);
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
        let child = self.push_selectable(SharedString::from(code), highlights, source_range);
        let code_block = wide_block(content, self.base)
            .my_2()
            .p_2()
            .rounded_md()
            .bg(self.code_bg)
            .font_family(self.mono.clone())
            .text_size(self.base)
            .child(child);
        let ix = self.blocks.len();
        self.blocks
            .push(wide_slot(code_block, content, ix, self.base));
    }
    fn flush_metadata(&mut self, source_range: Range<usize>) {
        let Some(body) = self.metadata.take() else {
            return;
        };
        let body = body.trim();
        if body.is_empty() {
            return;
        }
        let paint = MetaPaint {
            rule: self.rule,
            code_bg: self.code_bg,
            muted: self.muted,
            base: self.base,
            mono: self.mono.clone(),
            host: self.host.clone(),
            selection: self.selection_color,
        };
        let (element, plains, ranges) =
            render_metadata(body, source_range, &paint, self.plain.len());
        self.plain.extend(plains);
        self.source_ranges.extend(ranges);
        self.push_block(element);
    }

    fn in_table_cell(&self) -> bool {
        self.table.as_ref().is_some_and(|t| t.in_cell)
    }

    fn finish_table_cell(&mut self, range: Range<usize>) {
        let Some(table) = &mut self.table else {
            return;
        };
        let source_range = self.cell_source.take().unwrap_or(range);
        table.finish_cell(
            std::mem::take(&mut self.inline),
            std::mem::take(&mut self.highlights),
            source_range,
        );
    }

    fn flush_table(&mut self) {
        if let Some(table) = self.table.take() {
            let paint = TablePaint {
                rule: self.rule,
                code_bg: self.code_bg,
                base: self.base,
                host: self.host.clone(),
                selection: self.selection_color,
            };
            let (element, plains, ranges) = render_table(table, &paint, self.plain.len());
            self.plain.extend(plains);
            self.source_ranges.extend(ranges);
            self.push_block(element);
        }
    }

    fn code_color(&self, name: &str) -> Option<Hsla> {
        self.theme
            .syntax()
            .style_for_name(name)
            .and_then(|s| s.color)
    }
}
