//! Stateful host for markdown preview selection (plain block texts + caret).

use std::ops::Range;

use gpui::{Context, EventEmitter, SharedString};

use super::nav::HeadingNav;
use super::select::{
    Caret, PreviewSel, SelectMode, clamp_caret, line_range_at, select_all_carets,
    selected_markdown, word_range_at,
};

/// Fired when selection or scroll changes so the parent editor can re-render.
pub enum PreviewEvent {
    SelectionChanged,
    Scrolled,
}

#[derive(Default)]
pub struct PreviewState {
    /// Full markdown source (for clipboard).
    source: SharedString,
    /// Rendered plain text per selectable block (hit-test / highlight).
    blocks: Vec<SharedString>,
    /// Byte range of each block in `source` (copy markdown).
    source_ranges: Vec<Range<usize>>,
    sel: PreviewSel,
    nav: HeadingNav,
}

/// Left-click payload for preview selection.
pub(crate) struct PreviewClick {
    pub block: usize,
    pub offset: usize,
    pub count: usize,
    pub shift: bool,
}

impl EventEmitter<PreviewEvent> for PreviewState {}

impl PreviewState {
    pub(crate) fn sel(&self) -> &PreviewSel {
        &self.sel
    }

    pub(crate) fn nav(&self) -> &HeadingNav {
        &self.nav
    }

    /// Record the outline the last render showed (block index per heading).
    pub(crate) fn sync_outline(&mut self, blocks: Vec<usize>, outline_shown: bool) {
        self.nav.sync(blocks, outline_shown);
    }

    pub(crate) fn jump_to_heading(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.nav.jump(ix);
        cx.emit(PreviewEvent::Scrolled);
        cx.notify();
    }

    /// `[` / `]`: previous or next heading.
    pub(crate) fn step_heading(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.nav.step(delta);
        cx.emit(PreviewEvent::Scrolled);
        cx.notify();
    }

    /// Replace source + block map when the preview re-parses; clear selection if shape changes.
    pub(crate) fn sync_doc(
        &mut self,
        source: SharedString,
        blocks: Vec<SharedString>,
        source_ranges: Vec<Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        if self.source == source && self.blocks == blocks && self.source_ranges == source_ranges {
            return;
        }
        self.source = source;
        self.blocks = blocks;
        self.source_ranges = source_ranges;
        self.sel.clear();
        cx.notify();
    }

    /// Selected source markdown for the clipboard.
    pub(crate) fn selected_text(&self) -> String {
        selected_markdown(&self.source, &self.source_ranges, &self.sel)
    }

    pub(crate) fn select_all(&mut self, cx: &mut Context<Self>) {
        let Some((anchor, head)) = select_all_carets(&self.blocks) else {
            return;
        };
        self.sel.set_range(anchor, head, SelectMode::All, false);
        cx.emit(PreviewEvent::SelectionChanged);
        cx.notify();
    }

    pub(crate) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.sel.is_empty() && !self.sel.pending {
            return;
        }
        self.sel.clear();
        cx.emit(PreviewEvent::SelectionChanged);
        cx.notify();
    }

    pub(crate) fn mouse_down(&mut self, click: PreviewClick, cx: &mut Context<Self>) {
        if self.blocks.is_empty() {
            return;
        }
        let caret = clamp_caret(
            &self.blocks,
            Caret {
                block: click.block,
                offset: click.offset,
            },
        );
        let text = self
            .blocks
            .get(caret.block)
            .map(|s| s.as_ref())
            .unwrap_or("");
        match click.count {
            2 => {
                let range = word_range_at(text, caret.offset);
                self.sel.set_range(
                    Caret {
                        block: caret.block,
                        offset: range.start,
                    },
                    Caret {
                        block: caret.block,
                        offset: range.end,
                    },
                    SelectMode::Word(range),
                    true,
                );
            }
            3 => {
                let range = line_range_at(text, caret.offset);
                self.sel.set_range(
                    Caret {
                        block: caret.block,
                        offset: range.start,
                    },
                    Caret {
                        block: caret.block,
                        offset: range.end,
                    },
                    SelectMode::Line(range),
                    true,
                );
            }
            4.. => {
                if let Some((anchor, head)) = select_all_carets(&self.blocks) {
                    self.sel.set_range(anchor, head, SelectMode::All, true);
                }
            }
            _ if click.shift => {
                self.sel.extend_to(caret);
            }
            _ => {
                self.sel.set_caret(caret);
            }
        }
        cx.emit(PreviewEvent::SelectionChanged);
        cx.notify();
    }

    pub(crate) fn mouse_move(&mut self, block: usize, offset: usize, cx: &mut Context<Self>) {
        if !self.sel.pending {
            return;
        }
        let caret = clamp_caret(&self.blocks, Caret { block, offset });
        let text = self
            .blocks
            .get(caret.block)
            .map(|s| s.as_ref())
            .unwrap_or("");
        self.sel.set_head_drag(caret, text);
        cx.emit(PreviewEvent::SelectionChanged);
        cx.notify();
    }

    pub(crate) fn mouse_up(&mut self, cx: &mut Context<Self>) {
        if !self.sel.pending {
            return;
        }
        self.sel.pending = false;
        cx.emit(PreviewEvent::SelectionChanged);
        cx.notify();
    }
}
