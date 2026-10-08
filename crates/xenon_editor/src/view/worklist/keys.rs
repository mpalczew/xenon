//! List keyboard: arrows walk headers and items together; keys act on the selected row.

use super::{Cursor, EditorView, rows};
use crate::worklist_file::Target;
use crate::worklist_file::entries::parse;
use gpui::{Context, KeyDownEvent};

impl EditorView {
    pub(in crate::view) fn on_worklist_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.worklist_input_open() {
            return false;
        }
        let keys = event.keystroke.modifiers;
        let source = self.text();
        let parsed = parse(&source);
        let rows = rows::visible(&parsed.sections, &parsed.entries, &self.worklist_folded);
        let cursor = rows::settle(&rows, self.worklist_cursor, &parsed.entries);
        let item = |index: usize| (index < parsed.entries.len()).then_some(index);
        match (event.keystroke.key.as_str(), cursor) {
            ("up" | "down", _) if keys.alt => return false,
            ("up", _) => self.worklist_cursor = rows::step(&rows, cursor, -1),
            ("down", _) => self.worklist_cursor = rows::step(&rows, cursor, 1),
            ("enter", _) if keys.shift => self.worklist_start_new_section(cx),
            ("left", Cursor::Header(index)) => self.worklist_fold(index, Some(true), cx),
            ("right", Cursor::Header(index)) => self.worklist_fold(index, Some(false), cx),
            ("space", Cursor::Header(index)) => self.worklist_fold(index, None, cx),
            ("enter", Cursor::Header(index)) if keys.secondary() => {
                self.worklist_start_rename(index, cx)
            }
            ("enter", Cursor::Header(index)) => {
                let target = Target::of(&parsed.sections, Some(index));
                self.worklist_start_add(target, cx)
            }
            ("space", Cursor::Item(index)) if item(index).is_some() => {
                self.worklist_toggle(index, cx)
            }
            ("enter" | "delete", Cursor::Item(index)) if item(index).is_some() => {
                self.worklist_start_edit(index, cx)
            }
            ("enter", _) => self.worklist_start_add(Target::Top, cx),
            _ => return false,
        }
        cx.notify();
        true
    }
}
