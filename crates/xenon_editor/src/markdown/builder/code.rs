//! Fenced and indented code blocks for the preview builder.

use std::ops::Range;

use gpui::{HighlightStyle, Hsla, ParentElement, SharedString, Styled};

use super::Builder;
use crate::highlight;
use crate::markdown::{code_block_width, wide_block, wide_slot};

impl Builder {
    pub(super) fn flush_code(&mut self, source_range: Range<usize>) {
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
        let child = self.push_selectable(
            SharedString::from(code),
            highlights,
            Vec::new(),
            source_range,
        );
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

    fn code_color(&self, name: &str) -> Option<Hsla> {
        self.theme
            .syntax()
            .style_for_name(name)
            .and_then(|s| s.color)
    }
}
