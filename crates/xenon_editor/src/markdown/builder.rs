//! Event-driven markdown → gpui block builder (pulldown-cmark stream).

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, Hsla, IntoElement, ParentElement, Pixels,
    SharedString, StrikethroughStyle, Styled, StyledText, div, px,
};
use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use theme::Theme;

use super::table::{Table, TableCell, render_table};
use super::yaml::split_simple_yaml_pairs;
use super::{code_block_width, heading_size, wide_block};
use crate::highlight;

pub(super) struct Builder {
    theme: Arc<Theme>,
    accent: Hsla,
    muted: Hsla,
    code_bg: Hsla,
    rule: Hsla,
    base: Pixels,
    mono: String,
    blocks: Vec<AnyElement>,
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
}

pub(super) struct BuilderColors {
    pub accent: Hsla,
    pub muted: Hsla,
    pub code_bg: Hsla,
    pub rule: Hsla,
}

impl Builder {
    pub(super) fn new(
        theme: Arc<Theme>,
        colors: BuilderColors,
        base: Pixels,
        mono_family: &str,
    ) -> Self {
        Self {
            theme,
            accent: colors.accent,
            muted: colors.muted,
            code_bg: colors.code_bg,
            rule: colors.rule,
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
            metadata: None,
            table: None,
            def_title: None,
        }
    }

    pub(super) fn into_blocks(self) -> Vec<AnyElement> {
        self.blocks
    }

    fn push_meta_nl(&mut self) {
        if let Some(buffer) = &mut self.metadata {
            buffer.push('\n');
        }
    }

    pub(super) fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
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
            Tag::Item => self.inline.push_str("• "),
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
            TagEnd::MetadataBlock(_) => self.flush_metadata(),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => self.link = self.link.saturating_sub(1),
            TagEnd::FootnoteDefinition => {
                self.flush_block(px(f32::from(self.base) * 0.9), false, None)
            }
            TagEnd::DefinitionListTitle => {
                self.def_title = Some(std::mem::take(&mut self.inline).trim().to_string());
                self.highlights.clear();
            }
            TagEnd::DefinitionListDefinition => self.flush_definition(),
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

    fn flush_block(&mut self, size: Pixels, bold: bool, pad_left: Option<Pixels>) {
        if self.inline.trim().is_empty() {
            self.inline.clear();
            self.highlights.clear();
            return;
        }
        let text = SharedString::from(std::mem::take(&mut self.inline));
        let highlights = std::mem::take(&mut self.highlights);
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

    fn flush_metadata(&mut self) {
        let Some(body) = self.metadata.take() else {
            return;
        };
        let body = body.trim();
        if body.is_empty() {
            return;
        }
        let panel = split_simple_yaml_pairs(body)
            .map(|pairs| self.metadata_pairs_panel(pairs))
            .unwrap_or_else(|| self.metadata_raw_panel(body));
        self.blocks.push(panel);
    }

    fn metadata_pairs_panel(&self, pairs: Vec<(String, String)>) -> AnyElement {
        let mut panel = div()
            .my_3()
            .w_full()
            .min_w_0()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(self.rule)
            .bg(self.code_bg)
            .flex()
            .flex_col()
            .gap_2();
        for (key, value) in pairs {
            panel = panel.child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(f32::from(self.base) * 0.85))
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.muted)
                            .child(key),
                    )
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .whitespace_normal()
                            .text_size(self.base)
                            .child(value),
                    ),
            );
        }
        panel.into_any_element()
    }

    fn metadata_raw_panel(&self, body: &str) -> AnyElement {
        div()
            .my_3()
            .w_full()
            .min_w_0()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(self.rule)
            .bg(self.code_bg)
            .font_family(self.mono.clone())
            .text_size(px(f32::from(self.base) * 0.9))
            .whitespace_normal()
            .child(body.to_string())
            .into_any_element()
    }

    fn flush_definition(&mut self) {
        let title = self.def_title.take().unwrap_or_default().trim().to_string();
        let body = std::mem::take(&mut self.inline);
        let highlights = std::mem::take(&mut self.highlights);
        let body_trim = body.trim();
        if title.is_empty() && body_trim.is_empty() {
            return;
        }
        let mut block = div().my_1().w_full().min_w_0().flex().flex_col().gap_1();
        if !title.is_empty() {
            block = block.child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(self.base)
                    .child(title),
            );
        }
        if !body_trim.is_empty() {
            block = block.child(
                div()
                    .w_full()
                    .min_w_0()
                    .pl_4()
                    .whitespace_normal()
                    .text_size(self.base)
                    .child(StyledText::new(SharedString::from(body)).with_highlights(highlights)),
            );
        }
        self.blocks.push(block.into_any_element());
    }

    fn in_table_cell(&self) -> bool {
        self.table.as_ref().is_some_and(|t| t.in_cell)
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
        if let Some(table) = &mut self.table {
            let row = std::mem::take(&mut table.current);
            table.rows.push(row);
        }
    }

    fn flush_table(&mut self) {
        if let Some(table) = self.table.take() {
            self.blocks
                .push(render_table(table, self.rule, self.code_bg, self.base));
        }
    }

    fn code_color(&self, name: &str) -> Option<Hsla> {
        self.theme
            .syntax()
            .style_for_name(name)
            .and_then(|s| s.color)
    }
}
