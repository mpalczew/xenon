//! Markdown links: open the link under the cursor (F12, `gd`, ⌘-click) or a
//! clicked prose link. In-document `#anchors` scroll here; files and URLs go
//! up to the app as [`EditorEvent::OpenLink`].

use gpui::Context;

use super::{Content, EditorEvent, EditorView};
use crate::markdown::{self, LinkTarget};

impl EditorView {
    /// Destination of the markdown link (or path-like code span) at a
    /// zero-based row/column; `None` outside markdown or off a link.
    pub fn markdown_link_at(&self, row: u32, col: u32) -> Option<String> {
        let Content::Text(buffer) = &self.content else {
            return None;
        };
        if !self.is_markdown() {
            return None;
        }
        let rope = buffer.rope();
        let row = (row as usize).min(rope.len_lines().saturating_sub(1));
        let line_start = rope.line_to_char(row);
        let line_len = rope.line(row).len_chars();
        let byte = rope.char_to_byte(line_start + (col as usize).min(line_len));
        markdown::link_at(&buffer.text(), byte)
    }

    /// Follow a link destination written in this document.
    pub fn open_link(&mut self, dest: &str, cx: &mut Context<Self>) {
        let Some(target) = LinkTarget::parse(dest) else {
            return;
        };
        if let LinkTarget::Anchor(anchor) = &target {
            self.jump_to_anchor(anchor, cx);
            return;
        }
        cx.emit(EditorEvent::OpenLink {
            path: self.path().to_path_buf(),
            target,
        });
    }

    fn jump_to_anchor(&mut self, anchor: &str, cx: &mut Context<Self>) {
        let Content::Text(buffer) = &self.content else {
            return;
        };
        let Some(byte) = markdown::heading_offset(&buffer.text(), anchor) else {
            return;
        };
        let pos = buffer.rope().byte_to_char(byte);
        let row = buffer.rope().char_to_line(pos);
        self.set_cursor_position(row as u32, 0, cx);
        self.scroll_center_once = true;
        self.prose.layout_key = None;
    }
}
