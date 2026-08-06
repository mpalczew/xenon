//! Normal/visual character dispatch for vim mode.

use super::motion;
use super::paste::visual_excl_end;
use super::{
    FindKind, HandleResult, LastChange, Mode, Motion, Object, Operator, VimState, edited, handled,
};
use crate::buffer::Buffer;
use crate::selection;

impl VimState {
    // Key dispatch table: one match over the normal-mode alphabet.
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub(in crate::vim) fn handle_normal_char(
        &mut self,
        buffer: &mut Buffer,
        ch: char,
    ) -> HandleResult {
        if self.register_pending {
            self.register_pending = false;
            self.registers.set_pending(ch);
            return handled(false);
        }
        if let Some(kind) = self.awaiting_find.take() {
            let motion = match kind {
                FindKind::Find => Motion::Find {
                    ch,
                    before: false,
                    forward: true,
                },
                FindKind::Till => Motion::Find {
                    ch,
                    before: true,
                    forward: true,
                },
                FindKind::FindBack => Motion::Find {
                    ch,
                    before: false,
                    forward: false,
                },
                FindKind::TillBack => Motion::Find {
                    ch,
                    before: true,
                    forward: false,
                },
            };
            self.last_find = Some(motion.clone());
            let count = self.count.max(1);
            self.count = 0;
            return self.finish_motion(buffer, motion, count);
        }
        if self.awaiting_replace {
            self.awaiting_replace = false;
            return self.replace_char(buffer, ch);
        }
        if let Some(around) = self.awaiting_object {
            self.awaiting_object = None;
            if let Some(object) = parse_object(ch, around) {
                return self.apply_object(buffer, object);
            }
            self.clear_pending();
            return handled(false);
        }

        // Counts.
        if ch.is_ascii_digit() && (ch != '0' || self.count > 0) {
            self.count = self
                .count
                .saturating_mul(10)
                .saturating_add(ch as usize - '0' as usize);
            return handled(false);
        }

        // `g` prefix: `gg`, `gd`, `gJ` (invalid second key clears the prefix).
        if self.pending_g {
            self.pending_g = false;
            return match ch {
                'g' => {
                    if self.count > 0 {
                        let line = self.count.saturating_sub(1);
                        let row = line.min(buffer.rope().len_lines().saturating_sub(1));
                        buffer.set_cursor_position(row, 0);
                        self.count = 0;
                        handled(false)
                    } else {
                        self.count = 0;
                        self.do_motion(buffer, Motion::FileStart, 1)
                    }
                }
                'd' => {
                    self.count = 0;
                    HandleResult {
                        handled: true,
                        request_definition: true,
                        ..Default::default()
                    }
                }
                'J' => {
                    let lines = if self.count == 0 {
                        2
                    } else {
                        self.count.max(2)
                    };
                    if self.mode.is_visual() {
                        self.join_visual(buffer, false)
                    } else {
                        self.join_lines(buffer, lines, false)
                    }
                }
                _ => handled(false),
            };
        }
        // `z` prefix: `zz` centers the cursor line.
        if self.pending_z {
            self.pending_z = false;
            return match ch {
                'z' => {
                    self.count = 0;
                    HandleResult {
                        handled: true,
                        scroll_center: true,
                        ..Default::default()
                    }
                }
                _ => handled(false),
            };
        }

        let count = self.count.max(1);

        match ch {
            'g' => {
                self.pending_g = true;
                handled(false)
            }
            'z' => {
                self.pending_z = true;
                handled(false)
            }
            '"' => {
                self.register_pending = true;
                handled(false)
            }
            'i' if !self.mode.is_visual() && self.operator.is_none() => {
                self.count = 0;
                self.enter_insert(buffer);
                handled(false)
            }
            'a' if !self.mode.is_visual() && self.operator.is_none() => {
                // After current char, but never past EOL onto the next line.
                let c = buffer.cursor();
                let rope = buffer.rope();
                if c < rope.len_chars() && rope.char(c) != '\n' {
                    buffer.set_cursor_raw(c + 1);
                }
                self.enter_insert(buffer);
                handled(false)
            }
            // Visual-block insert at left / right edge of the rectangle.
            'I' if self.mode == Mode::VisualBlock => self.block_insert(buffer, true),
            'A' if self.mode == Mode::VisualBlock => self.block_insert(buffer, false),
            'I' if !self.mode.is_visual() => {
                let pos = motion::apply(buffer.rope(), buffer.cursor(), &Motion::FirstNonBlank, 1);
                buffer.set_cursor_raw(pos);
                self.enter_insert(buffer);
                handled(false)
            }
            'A' if !self.mode.is_visual() => {
                // Insert at exclusive EOL (before the newline, if any).
                let pos = motion::line_end_exclusive(buffer.rope(), buffer.cursor());
                buffer.set_cursor_raw(pos);
                self.enter_insert(buffer);
                handled(false)
            }
            'o' if !self.mode.is_visual() => {
                // Newline + insert text = one undo step until Esc.
                buffer.set_undo_group(true);
                let end = motion::line_end_exclusive(buffer.rope(), buffer.cursor());
                buffer.set_cursor_raw(end);
                buffer.replace_selection("\n");
                self.enter_insert(buffer);
                edited()
            }
            'O' if !self.mode.is_visual() => {
                buffer.set_undo_group(true);
                let start = motion::apply(buffer.rope(), buffer.cursor(), &Motion::LineStart, 1);
                buffer.set_cursor_raw(start);
                buffer.replace_selection("\n");
                buffer.set_cursor_raw(start);
                self.enter_insert(buffer);
                edited()
            }
            'v' => {
                if self.mode == Mode::Visual {
                    buffer.clear_selection();
                    self.block_anchor = None;
                    self.mode = Mode::Normal;
                } else {
                    // From block: drop to char visual on the head char.
                    self.block_anchor = None;
                    self.mode = Mode::Visual;
                    let c = buffer.cursor();
                    let end = visual_excl_end(buffer, c);
                    buffer.set_selection(c, end);
                }
                self.clear_pending();
                handled(false)
            }
            'V' => {
                if self.mode == Mode::VisualLine {
                    buffer.clear_selection();
                    self.block_anchor = None;
                    self.mode = Mode::Normal;
                } else {
                    self.block_anchor = None;
                    self.mode = Mode::VisualLine;
                    let range = selection::line_range_at(buffer.rope(), buffer.cursor());
                    buffer.set_selection(range.start, range.end);
                }
                self.clear_pending();
                handled(false)
            }
            'h' => self.do_motion(buffer, Motion::Left, count),
            'j' => self.do_motion(buffer, Motion::Down, count),
            'k' => self.do_motion(buffer, Motion::Up, count),
            'l' => self.do_motion(buffer, Motion::Right, count),
            'w' => self.do_motion(buffer, Motion::WordForward, count),
            'W' => self.do_motion(buffer, Motion::WORDForward, count),
            'b' => self.do_motion(buffer, Motion::WordBackward, count),
            'B' => self.do_motion(buffer, Motion::WORDBackward, count),
            'e' => self.do_motion(buffer, Motion::WordEnd, count),
            '0' => self.do_motion(buffer, Motion::LineStart, 1),
            '^' => self.do_motion(buffer, Motion::FirstNonBlank, 1),
            '$' => self.do_motion(buffer, Motion::LineEnd, 1),
            '%' => self.do_percent(buffer),
            'G' => {
                if self.count == 0 {
                    self.count = 0;
                    self.do_motion(buffer, Motion::FileEnd, 1)
                } else {
                    let line = self.count.saturating_sub(1);
                    let row = line.min(buffer.rope().len_lines().saturating_sub(1));
                    buffer.set_cursor_position(row, 0);
                    self.count = 0;
                    handled(false)
                }
            }
            // Join lines (`J`); count is total lines fused (default 2).
            'J' if self.mode.is_visual() => self.join_visual(buffer, true),
            'J' => {
                let lines = if self.count == 0 {
                    2
                } else {
                    self.count.max(2)
                };
                self.join_lines(buffer, lines, true)
            }
            // `D` = `d$`, `C` = `c$`, `Y` = `yy`, `S` = `cc`.
            'D' if !self.mode.is_visual() => {
                self.count = 0;
                self.operator = Some(Operator::Delete);
                self.finish_motion(buffer, Motion::LineEnd, 1)
            }
            'C' if !self.mode.is_visual() => {
                self.count = 0;
                self.operator = Some(Operator::Change);
                self.finish_motion(buffer, Motion::LineEnd, 1)
            }
            'Y' if !self.mode.is_visual() => {
                self.operator = Some(Operator::Yank);
                self.op_or_line(buffer, Operator::Yank, count, 'y')
            }
            'S' if !self.mode.is_visual() && self.operator.is_none() => {
                self.operator = Some(Operator::Change);
                self.op_or_line(buffer, Operator::Change, count, 'c')
            }
            // Indent / outdent / reindent.
            '>' if self.mode.is_visual() => self.visual_operator(buffer, Operator::Indent),
            '<' if self.mode.is_visual() => self.visual_operator(buffer, Operator::Outdent),
            '=' if self.mode.is_visual() => self.visual_operator(buffer, Operator::Reindent),
            '>' => self.op_or_line(buffer, Operator::Indent, count, '>'),
            '<' => self.op_or_line(buffer, Operator::Outdent, count, '<'),
            '=' => self.op_or_line(buffer, Operator::Reindent, count, '='),
            'f' => {
                self.awaiting_find = Some(FindKind::Find);
                handled(false)
            }
            't' => {
                self.awaiting_find = Some(FindKind::Till);
                handled(false)
            }
            'F' => {
                self.awaiting_find = Some(FindKind::FindBack);
                handled(false)
            }
            'T' => {
                self.awaiting_find = Some(FindKind::TillBack);
                handled(false)
            }
            ';' => {
                if let Some(m) = self.last_find.clone() {
                    self.finish_motion(buffer, m, count)
                } else {
                    handled(false)
                }
            }
            ',' => {
                if let Some(Motion::Find {
                    ch,
                    before,
                    forward,
                }) = self.last_find.clone()
                {
                    self.finish_motion(
                        buffer,
                        Motion::Find {
                            ch,
                            before,
                            forward: !forward,
                        },
                        count,
                    )
                } else {
                    handled(false)
                }
            }
            'd' => self.op_or_line(buffer, Operator::Delete, count, 'd'),
            'c' => self.op_or_line(buffer, Operator::Change, count, 'c'),
            'y' => self.op_or_line(buffer, Operator::Yank, count, 'y'),
            'x' if self.mode.is_visual() => self.visual_operator(buffer, Operator::Delete),
            'X' if self.mode.is_visual() => self.visual_operator(buffer, Operator::Delete),
            'x' => self.delete_chars(buffer, count, false),
            'X' => self.delete_chars(buffer, count, true),
            'p' if self.mode.is_visual() => self.visual_paste(buffer, false),
            'P' if self.mode.is_visual() => self.visual_paste(buffer, true),
            'p' => self.paste(buffer, false),
            'P' => self.paste(buffer, true),
            'o' if self.mode.is_visual() => self.visual_swap_ends(buffer),
            'r' if self.mode.is_visual() => {
                // Visual `rX` replaces selection with char — treat as await then expand.
                self.awaiting_replace = true;
                handled(false)
            }
            'r' => {
                self.awaiting_replace = true;
                handled(false)
            }
            ':' => {
                self.begin_ex(buffer);
                handled(false)
            }
            'u' => {
                // Each `u` is one change; count undoes that many (vim `3u`).
                let mut any = false;
                for _ in 0..count {
                    if !buffer.undo() {
                        break;
                    }
                    any = true;
                }
                self.count = 0;
                HandleResult {
                    handled: true,
                    edited: any,
                    ..Default::default()
                }
            }
            '.' => self.repeat_last(buffer),
            '/' => {
                self.begin_search(buffer, true);
                handled(false)
            }
            '?' => {
                self.begin_search(buffer, false);
                handled(false)
            }
            'n' => self.search_again(buffer, true),
            'N' => self.search_again(buffer, false),
            '*' => self.search_word(buffer),
            's' if self.mode.is_visual() => self.visual_operator(buffer, Operator::Change),
            // Normal `s` = change one char (like `cl`).
            's' if !self.mode.is_visual() && self.operator.is_none() => {
                let n = count.max(1);
                self.count = 0;
                self.last_change = Some(LastChange::DeleteChar { count: n });
                let cursor = buffer.cursor();
                let end = (cursor + n).min(buffer.rope().len_chars());
                if cursor < end {
                    let text = buffer.rope().slice(cursor..end).to_string();
                    self.registers.delete(&text, false);
                    buffer.set_selection(cursor, end);
                    buffer.set_undo_group(true);
                    buffer.delete_selection();
                }
                self.enter_insert(buffer);
                edited()
            }
            // i/a as object prefix after operator
            'i' if self.operator.is_some() => {
                self.awaiting_object = Some(false);
                handled(false)
            }
            'a' if self.operator.is_some() => {
                self.awaiting_object = Some(true);
                handled(false)
            }
            _ => handled(false),
        }
    }

    /// Bare `%` → match bracket; `N%` → N percent of the file.
    fn do_percent(&mut self, buffer: &mut Buffer) -> HandleResult {
        if self.count > 0 && self.count != usize::MAX {
            let pct = self.count.min(100);
            self.count = 0;
            self.do_motion(buffer, Motion::Percent { pct }, 1)
        } else {
            self.count = 0;
            self.do_motion(buffer, Motion::MatchPair, 1)
        }
    }
}

fn parse_object(ch: char, around: bool) -> Option<Object> {
    Some(match ch {
        'w' => Object::Word { around },
        '"' | '\'' | '`' => Object::Quote { ch, around },
        '(' | ')' | 'b' => Object::Pair {
            open: '(',
            close: ')',
            around,
        },
        '[' | ']' => Object::Pair {
            open: '[',
            close: ']',
            around,
        },
        '{' | '}' | 'B' => Object::Pair {
            open: '{',
            close: '}',
            around,
        },
        _ => return None,
    })
}
