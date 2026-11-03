//! Prose modes, Paper and per-file prefs.

use gpui::Context;
use xenon_settings::{MarkdownMode, ProseFilePrefs};

use super::super::{Content, EditorView};
use crate::prose::layout::Hit;

impl EditorView {
    pub fn is_md_prose_capable(&self) -> bool {
        self.is_markdown() && matches!(self.content, Content::Text(_)) && !self.worklist_document
    }

    /// Load this file's saved mode, Paper and comment fold once.
    pub(in crate::view) fn ensure_prose_prefs(&mut self) {
        if self.prose.loaded_prefs || !self.is_md_prose_capable() {
            return;
        }
        self.prose.loaded_prefs = true;
        let prefs = xenon_settings::prose_file_prefs(self.path());
        self.prose.mode = prefs.mode;
        self.prose.paper = prefs.paper;
        self.prose.comments_open = prefs.comments_open;
    }

    pub(super) fn save_prose_prefs(&self) {
        let _ = xenon_settings::set_prose_file_prefs(
            self.path(),
            ProseFilePrefs {
                mode: self.prose.mode,
                paper: self.prose.paper,
                comments_open: self.prose.comments_open,
            },
        );
    }

    pub fn markdown_mode(&self) -> MarkdownMode {
        if self.is_md_prose_capable() {
            self.prose.mode
        } else {
            MarkdownMode::Source
        }
    }

    pub fn set_markdown_mode(&mut self, mode: MarkdownMode, cx: &mut Context<Self>) {
        if !self.is_md_prose_capable() {
            return;
        }
        self.ensure_prose_prefs();
        self.end_typing();
        let (had_prose, had_source) = (self.shows_prose_pane(), self.shows_source_pane());
        let anchor = self.top_anchor();
        if mode == MarkdownMode::Split && self.prose.mode == MarkdownMode::Split {
            self.prose.active_prose = !self.prose.active_prose;
        } else {
            self.prose.active_prose = true;
        }
        self.prose.mode = mode;
        self.prose.layout_key = None;
        // A pane that just appeared opens where the reader was; one that stays keeps its scroll.
        // Without an anchor it falls back to bringing the caret into view.
        if self.shows_prose_pane() && !had_prose {
            self.prose.anchor = anchor;
            self.prose.last_cursor = None;
        }
        if self.shows_source_pane() && !had_source {
            self.scroll_anchor = anchor;
            self.last_cursor = None;
        }
        self.context_menu = None;
        self.save_prose_prefs();
        cx.notify();
    }

    /// Char offset at the top of the pane the reader is looking at.
    pub(in crate::view) fn top_anchor(&self) -> Option<usize> {
        if self.prose_active() {
            let layout = self.prose.layout.as_ref()?;
            let y = self.prose.scroll;
            let bi = layout.block_at_y(y);
            let bl = layout.blocks.get(bi)?;
            return Some(match layout.hit(bl.x, y.max(bl.y + bl.text_top)) {
                Hit::Pos(pos) => pos,
                Hit::Check(_) | Hit::Chip => layout.proj.block(bi).src.start,
            });
        }
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        let click = self.click_layout.as_ref()?;
        let visual = (click.scroll_top / click.line_height).floor().max(0.) as usize;
        let (line, col) = match click.wrap_rows.as_deref() {
            Some(rows) => crate::element::buffer_at(rows, visual, 0),
            None => (visual, 0),
        };
        let rope = buffer.rope();
        let line = line.min(rope.len_lines().saturating_sub(1));
        Some(rope.line_to_char(line) + col)
    }

    /// Whether keystrokes and typed text act through the prose projection.
    pub(in crate::view) fn prose_active(&self) -> bool {
        self.is_md_prose_capable()
            && match self.prose.mode {
                MarkdownMode::Prose => true,
                MarkdownMode::Split => self.prose.active_prose,
                MarkdownMode::Source => false,
            }
    }

    /// ⌘+ / ⌘− scale the prose body, not the editor font, while prose has keys.
    pub fn zooms_prose(&self) -> bool {
        self.prose_active()
    }

    pub(in crate::view) fn shows_prose_pane(&self) -> bool {
        self.is_md_prose_capable() && self.prose.mode != MarkdownMode::Source
    }

    pub(in crate::view) fn shows_source_pane(&self) -> bool {
        !self.is_md_prose_capable() || self.prose.mode != MarkdownMode::Prose
    }
}
