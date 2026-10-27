//! Prose keyboard handling.

use gpui::{Context, KeyDownEvent};

use super::super::{Content, EditorView};
use crate::prose::edit as prose_edit;

impl EditorView {
    /// Key handling in prose mode. Returns true when consumed.
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub(in crate::view) fn on_prose_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let k = &event.keystroke;
        let mods = &k.modifiers;
        let extend = mods.shift;
        let key = k.key.as_str();
        let Some(proj) = self.fresh_projection(cx) else {
            return false;
        };
        let len = self.rope_len();
        let Content::Text(buffer) = &self.content else {
            return false;
        };
        let cursor = buffer.cursor();
        let selection = buffer.selection_range();
        if key == "escape" {
            if self.prose.chrome_hidden {
                self.toggle_full_screen(cx);
                return true;
            }
            if selection.is_some() {
                self.move_to(cursor, false, false, cx);
                return true;
            }
            return false;
        }
        if mods.secondary() {
            return match key {
                "left" => {
                    self.row_edge(false, extend, cx);
                    true
                }
                "right" => {
                    self.row_edge(true, extend, cx);
                    true
                }
                "up" => {
                    self.move_to(0, extend, false, cx);
                    true
                }
                "down" => {
                    self.move_to(len, extend, false, cx);
                    true
                }
                "enter" => {
                    self.toggle_task(cx);
                    true
                }
                _ => false,
            };
        }
        if mods.alt && matches!(key, "left" | "right") {
            self.word_move(if key == "left" { -1 } else { 1 }, extend, cx);
            return true;
        }
        if mods.alt || mods.control || mods.platform {
            return false;
        }
        match key {
            "left" => {
                let to = match (&selection, extend) {
                    (Some(r), false) => r.start,
                    _ => proj.step(cursor, -1, len),
                };
                self.move_to(to, extend, false, cx);
            }
            "right" => {
                let to = match (&selection, extend) {
                    (Some(r), false) => r.end,
                    _ => proj.step(cursor, 1, len),
                };
                self.move_to(to, extend, false, cx);
            }
            "up" => self.vertical(-1, extend, cx),
            "down" => self.vertical(1, extend, cx),
            "home" => self.row_edge(false, extend, cx),
            "end" => self.row_edge(true, extend, cx),
            "backspace" => {
                self.end_typing();
                if let Some(r) = selection {
                    self.prose_replace(r, "", cx);
                } else if let Some(r) = prose_edit::backspace(self.rope(), &proj, cursor) {
                    self.prose_replace(r, "", cx);
                }
            }
            "delete" => {
                self.end_typing();
                if let Some(r) = selection {
                    self.prose_replace(r, "", cx);
                } else if let Some(r) = prose_edit::delete(self.rope(), &proj, cursor) {
                    self.prose_replace(r, "", cx);
                }
            }
            "enter" => {
                self.end_typing();
                if let Some(r) = selection {
                    self.prose_replace(r, "", cx);
                    return self.on_prose_key(event, cx);
                }
                let block = proj.block(proj.doc.block_at(cursor));
                if block.kind == crate::prose::model::BlockKind::Chip
                    && (cursor == block.src.start || cursor == block.src.end)
                    && block.chip.is_some()
                {
                    self.toggle_comments_fold(cx);
                    return true;
                }
                let plan = prose_edit::enter(self.rope(), &proj, cursor, extend);
                self.prose_replace(plan.range, &plan.text, cx);
            }
            "tab" => {
                self.end_typing();
                let block = proj.block(proj.doc.block_at(cursor));
                if let Some(first) = block.pieces.iter().find_map(|p| match p.kind {
                    crate::prose::model::PieceKind::Lead(
                        crate::prose::model::LeadKind::Bullet
                        | crate::prose::model::LeadKind::Number,
                    ) => Some(p.src.clone()),
                    _ => None,
                }) {
                    let line_start = self
                        .rope()
                        .line_to_char(self.rope().char_to_line(first.start));
                    if extend {
                        let spaces = (line_start..first.start)
                            .take_while(|i| self.rope().char(*i) == ' ')
                            .count()
                            .min(first.end - first.start);
                        if spaces > 0 {
                            self.prose_replace(line_start..line_start + spaces, "", cx);
                        }
                    } else {
                        let width = first.end - first.start;
                        self.prose_replace(line_start..line_start, &" ".repeat(width), cx);
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Flip the task box on the caret's item.
    pub fn toggle_task(&mut self, cx: &mut Context<Self>) {
        let Some(proj) = self.fresh_projection(cx) else {
            return;
        };
        let Content::Text(buffer) = &self.content else {
            return;
        };
        if let Some(plan) = prose_edit::toggle_task(buffer.rope(), &proj, buffer.cursor()) {
            let keep = buffer.cursor();
            self.end_typing();
            self.prose_replace(plan.range, &plan.text, cx);
            if let Content::Text(buffer) = &mut self.content {
                buffer.set_cursor_raw(keep);
            }
        }
    }
}
