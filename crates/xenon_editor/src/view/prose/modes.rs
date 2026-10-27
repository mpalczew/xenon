//! Prose modes, Paper, full screen and per-file prefs.

use gpui::Context;
use xenon_settings::{MarkdownMode, ProseFilePrefs};

use super::super::{Content, EditorView};

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
        if mode == MarkdownMode::Split && self.prose.mode == MarkdownMode::Split {
            self.prose.active_prose = !self.prose.active_prose;
        } else {
            self.prose.active_prose = true;
        }
        self.prose.mode = mode;
        self.prose.layout_key = None;
        self.prose.last_cursor = None;
        self.last_cursor = None;
        self.context_menu = None;
        self.save_prose_prefs();
        cx.notify();
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

    pub(in crate::view) fn shows_prose_pane(&self) -> bool {
        self.is_md_prose_capable() && self.prose.mode != MarkdownMode::Source
    }

    pub(in crate::view) fn shows_source_pane(&self) -> bool {
        !self.is_md_prose_capable() || self.prose.mode != MarkdownMode::Prose
    }
}
