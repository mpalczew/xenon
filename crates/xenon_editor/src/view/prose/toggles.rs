//! Paper, full screen and comment fold toggles.

use gpui::Context;

use super::super::{EditorEvent, EditorView};

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

    pub fn toggle_full_screen(&mut self, cx: &mut Context<Self>) {
        if !self.is_md_prose_capable() {
            return;
        }
        let on = !self.prose.chrome_hidden;
        self.set_chrome_hidden(on, cx);
        cx.emit(EditorEvent::FullScreen { on });
    }

    /// The shell hid or restored its chrome; hide the status strip with it.
    pub fn set_chrome_hidden(&mut self, hidden: bool, cx: &mut Context<Self>) {
        if self.prose.chrome_hidden != hidden {
            self.prose.chrome_hidden = hidden;
            cx.notify();
        }
    }

    pub fn is_paper(&self) -> bool {
        self.prose.paper
    }

    pub fn comments_open(&self) -> bool {
        self.prose.comments_open
    }

    pub fn is_full_screen(&self) -> bool {
        self.prose.chrome_hidden
    }

    pub fn toggle_comments_fold(&mut self, cx: &mut Context<Self>) {
        self.prose.comments_open = !self.prose.comments_open;
        self.prose.layout_key = None;
        self.save_prose_prefs();
        cx.notify();
    }
}
