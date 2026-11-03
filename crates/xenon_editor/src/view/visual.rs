//! Visual-test hooks for seeding deterministic editor chrome.

use gpui::{Context, Pixels, Point};

use super::{Content, EditorView};
use crate::edit::EditCommand;

impl EditorView {
    pub fn visual_remote(
        path: std::path::PathBuf,
        text: &str,
        unavailable: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::from_content(
            Content::Text(crate::Buffer::from_remote(path, text)),
            true,
            cx,
        );
        view.remote = Some(super::remote::RemoteDocument::fixture(text));
        if unavailable {
            view.disk_alert = super::DiskAlert::Unavailable;
        }
        view
    }
    pub fn visual_insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.apply(EditCommand::Insert(text.to_string()));
        self.recompute_highlights();
        cx.notify();
    }

    /// Buffer line at the top of the pane that has keys.
    pub fn visual_top_line(&self) -> Option<usize> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        Some(buffer.rope().char_to_line(self.top_anchor()?))
    }

    /// Scroll the source pane so `line` (one visual row each) is on top.
    pub fn visual_scroll_source(&mut self, line: usize, cx: &mut Context<Self>) {
        if let Some(click) = &self.click_layout {
            self.scroll_top = click.line_height * (line as f32);
        }
        cx.notify();
    }

    pub fn visual_open_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.context_menu = Some(position);
        cx.notify();
    }
}
