//! Paper and comment fold toggles.

use gpui::Context;

use super::super::EditorView;

impl EditorView {
    pub fn toggle_paper(&mut self, cx: &mut Context<Self>) {
        if !self.is_md_prose_capable() {
            return;
        }
        self.prose.paper = !self.prose.paper;
        self.prose.layout_key = None;
        self.save_prose_prefs();
        cx.notify();
    }

    pub fn is_paper(&self) -> bool {
        self.prose.paper
    }

    pub fn comments_open(&self) -> bool {
        self.prose.comments_open
    }

    pub fn toggle_comments_fold(&mut self, cx: &mut Context<Self>) {
        self.prose.comments_open = !self.prose.comments_open;
        self.prose.layout_key = None;
        self.save_prose_prefs();
        cx.notify();
    }
}
