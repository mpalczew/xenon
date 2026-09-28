//! Definition-list items (`term` / `: definition`) for the preview builder.

use std::ops::Range;

use gpui::{FontWeight, ParentElement, SharedString, Styled, div};

use super::Builder;

impl Builder {
    pub(super) fn flush_definition(&mut self, body_source: Range<usize>) {
        let title = self.def_title.take().unwrap_or_default().trim().to_string();
        let title_source = self.def_title_source.take();
        let body = std::mem::take(&mut self.inline);
        let highlights = std::mem::take(&mut self.highlights);
        let body_trim = body.trim();
        if title.is_empty() && body_trim.is_empty() {
            return;
        }
        let mut block = div().my_1().w_full().min_w_0().flex().flex_col().gap_1();
        if !title.is_empty() {
            let src = title_source.unwrap_or_else(|| body_source.clone());
            let title_el = self.push_selectable(SharedString::from(title), Vec::new(), src);
            block = block.child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(self.base)
                    .child(title_el),
            );
        }
        if !body_trim.is_empty() {
            let body_el = self.push_selectable(SharedString::from(body), highlights, body_source);
            block = block.child(
                div()
                    .w_full()
                    .min_w_0()
                    .pl_4()
                    .whitespace_normal()
                    .text_size(self.base)
                    .child(body_el),
            );
        }
        self.push_block(block);
    }
}
