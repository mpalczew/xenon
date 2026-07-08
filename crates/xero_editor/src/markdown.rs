//! Render a markdown document to gpui elements for the preview pane. Block
//! structure comes from `pulldown-cmark`; inline emphasis/links/code use
//! `StyledText` highlights; fenced code blocks are syntax-highlighted via the
//! tree-sitter highlighters, keyed by the fence language.

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, StyledText, div, px,
};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};
use theme::{ActiveTheme, Theme};

use crate::highlight;

const MONO: &str = "Menlo";

/// Render markdown `source` into a scrollable column of block elements sized
/// relative to `base`.
pub fn render(source: &str, base: Pixels, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let colors = theme.colors().clone();
    let mut builder = Builder {
        theme,
        accent: colors.text_accent,
        code_bg: colors.surface_background,
        rule: colors.border,
        base,
        blocks: Vec::new(),
        inline: String::new(),
        highlights: Vec::new(),
        bold: 0,
        italic: 0,
        link: 0,
        code: None,
    };
    for event in Parser::new(source) {
        builder.event(event);
    }
    div()
        .id("md-preview")
        .size_full()
        .overflow_y_scroll()
        .p_4()
        .text_size(base)
        .text_color(colors.text)
        .bg(colors.editor_background)
        .children(builder.blocks)
        .into_any_element()
}

struct Builder {
    theme: Arc<Theme>,
    accent: Hsla,
    code_bg: Hsla,
    rule: Hsla,
    base: Pixels,
    blocks: Vec<AnyElement>,
    // Current inline run being accumulated for the open block.
    inline: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    bold: u32,
    italic: u32,
    link: u32,
    // When inside a fenced block: (language, collected source).
    code: Option<(String, String)>,
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
            Tag::Link { .. } => self.link += 1,
            Tag::Item => self.inline.push_str("• "),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush_block(self.base, false, None),
            TagEnd::Heading(level) => self.flush_block(heading_size(self.base, level), true, None),
            TagEnd::Item => self.flush_block(self.base, false, Some(px(16.))),
            TagEnd::CodeBlock => self.flush_code(),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Link => self.link = self.link.saturating_sub(1),
            _ => {}
        }
    }

    /// The inline style implied by the currently open emphasis/strong/link tags.
    fn inline_style(&self) -> Option<HighlightStyle> {
        if self.bold == 0 && self.italic == 0 && self.link == 0 {
            return None;
        }
        Some(HighlightStyle {
            font_weight: (self.bold > 0).then_some(FontWeight::BOLD),
            font_style: (self.italic > 0).then_some(FontStyle::Italic),
            color: (self.link > 0).then_some(self.accent),
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
            div()
                .my_2()
                .p_2()
                .rounded_md()
                .bg(self.code_bg)
                .font_family(MONO)
                .text_size(self.base)
                .child(StyledText::new(SharedString::from(code)).with_highlights(highlights))
                .into_any_element(),
        );
    }

    fn code_color(&self, name: &str) -> Option<Hsla> {
        self.theme
            .syntax()
            .style_for_name(name)
            .and_then(|style| style.color)
    }
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
