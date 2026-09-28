//! Heading outline and scroll position for the markdown preview: which heading
//! is current, and where `[` / `]` or an outline click should scroll.

use gpui::{Pixels, ScrollHandle, SharedString, point, px};

/// Gap kept above a heading after jumping to it.
const JUMP_GAP: f32 = 16.;
/// A heading counts as current once its top passes this far below the viewport top.
const ACTIVE_LINE: f32 = 32.;

/// One heading in the preview outline.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: SharedString,
    /// Index of the heading among the scroll container's children.
    pub block: usize,
}

#[derive(Default)]
pub(crate) struct HeadingNav {
    scroll: ScrollHandle,
    blocks: Vec<usize>,
    outline_shown: bool,
}

impl HeadingNav {
    pub(crate) fn scroll(&self) -> &ScrollHandle {
        &self.scroll
    }

    pub(crate) fn sync(&mut self, blocks: Vec<usize>, outline_shown: bool) {
        self.blocks = blocks;
        self.outline_shown = outline_shown;
    }

    /// Whether the outline fits beside a column that needs `needed` pixels.
    /// `None` until the preview has been laid out once.
    pub(crate) fn outline_fits(&self, needed: Pixels, outline_w: Pixels) -> Option<bool> {
        let width = self.scroll.bounds().size.width;
        if width <= px(0.) {
            return None;
        }
        let total = if self.outline_shown {
            width + outline_w
        } else {
            width
        };
        Some(total >= needed)
    }

    /// Outline index of the heading the reader is in.
    pub(crate) fn active(&self, blocks: &[usize]) -> Option<usize> {
        let max = self.scroll.max_offset().y;
        if max > px(0.) && self.scroll.offset().y <= -max + px(1.) {
            return blocks.len().checked_sub(1);
        }
        let line = self.scroll.bounds().top() + px(ACTIVE_LINE);
        active_index(&self.visible_tops(blocks), line)
    }

    /// Scroll so outline entry `ix` sits at the top.
    pub(crate) fn jump(&self, ix: usize) {
        let Some(&block) = self.blocks.get(ix) else {
            return;
        };
        let Some(item) = self.scroll.bounds_for_item(block) else {
            self.scroll.scroll_to_item(block);
            return;
        };
        let max = self.scroll.max_offset().y.max(px(0.));
        let top = item.top() - self.scroll.bounds().top() - px(JUMP_GAP);
        let y = (-top).clamp(-max, px(0.));
        self.scroll.set_offset(point(self.scroll.offset().x, y));
    }

    /// Move to the previous (`-1`) or next (`1`) heading.
    pub(crate) fn step(&self, delta: i32) {
        let active = self.active(&self.blocks);
        let line = self.scroll.bounds().top() + px(ACTIVE_LINE);
        let at_top = active
            .and_then(|ix| self.visible_tops(&self.blocks)[ix])
            .is_some_and(|top| top >= line - px(ACTIVE_LINE));
        if let Some(target) = step_target(active, at_top, delta, self.blocks.len()) {
            self.jump(target);
        }
    }

    fn visible_tops(&self, blocks: &[usize]) -> Vec<Option<Pixels>> {
        let offset = self.scroll.offset().y;
        blocks
            .iter()
            .map(|&block| {
                self.scroll
                    .bounds_for_item(block)
                    .map(|item| item.top() + offset)
            })
            .collect()
    }
}

/// Last heading whose top has passed `line`; the first heading before any has.
fn active_index(tops: &[Option<Pixels>], line: Pixels) -> Option<usize> {
    let passed = tops
        .iter()
        .rposition(|top| top.is_some_and(|top| top <= line));
    passed.or((!tops.is_empty()).then_some(0))
}

/// Heading to jump to. Going back from the middle of a section returns to its
/// own heading first.
fn step_target(active: Option<usize>, at_top: bool, delta: i32, len: usize) -> Option<usize> {
    let last = len.checked_sub(1)?;
    let Some(active) = active else {
        return Some(0);
    };
    match delta.signum() {
        1 => (active < last).then_some(active + 1),
        -1 if !at_top => Some(active),
        -1 => active.checked_sub(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_is_last_heading_above_line() {
        let tops = [Some(px(-200.)), Some(px(10.)), Some(px(400.))];
        assert_eq!(active_index(&tops, px(32.)), Some(1));
    }

    #[test]
    fn active_defaults_to_first_heading() {
        let tops = [Some(px(100.)), Some(px(400.))];
        assert_eq!(active_index(&tops, px(32.)), Some(0));
        assert_eq!(active_index(&[], px(32.)), None);
    }

    #[test]
    fn next_stops_at_last() {
        assert_eq!(step_target(Some(0), true, 1, 3), Some(1));
        assert_eq!(step_target(Some(2), true, 1, 3), None);
    }

    #[test]
    fn previous_returns_to_section_heading_first() {
        assert_eq!(step_target(Some(2), false, -1, 3), Some(2));
        assert_eq!(step_target(Some(2), true, -1, 3), Some(1));
        assert_eq!(step_target(Some(0), true, -1, 3), None);
    }

    #[test]
    fn no_headings_no_target() {
        assert_eq!(step_target(None, true, 1, 0), None);
    }
}
