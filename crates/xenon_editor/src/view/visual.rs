//! Visual-test hooks for seeding deterministic editor chrome.

use gpui::{Context, Pixels, Point};

use super::{Content, EditorView};
use crate::edit::EditCommand;

impl EditorView {
    pub fn visual_insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.apply(EditCommand::Insert(text.to_string()));
        self.recompute_highlights();
        cx.notify();
    }

    pub fn visual_open_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.context_menu = Some(position);
        cx.notify();
    }
}
