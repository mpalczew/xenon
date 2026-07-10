//! Normal/visual character dispatch for vim mode.

use super::motion;
use super::{
    FindKind, HandleResult, Mode, Motion, Object, Operator, SearchDraft, VimState, edited, handled,
};
use crate::buffer::Buffer;
use crate::selection;

impl VimState {
    // Key dispatch table: one match over the normal-mode alphabet.
    #[allow(clippy::too_many_lines)]
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

        let count = self.count.max(1);

        match ch {
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
                let c = buffer.cursor();
                if c < buffer.rope().len_chars() {
                    buffer.set_cursor_raw(c + 1);
                }
                self.enter_insert(buffer);
                handled(false)
            }
            'I' if !self.mode.is_visual() => {
                let pos = motion::apply(buffer.rope(), buffer.cursor(), &Motion::FirstNonBlank, 1);
                buffer.set_cursor_raw(pos);
                self.enter_insert(buffer);
                handled(false)
            }
            'A' if !self.mode.is_visual() => {
                let pos = motion::apply(buffer.rope(), buffer.cursor(), &Motion::LineEnd, 1);
                buffer.set_cursor_raw(pos);
                self.enter_insert(buffer);
                handled(false)
            }
            'o' if !self.mode.is_visual() => {
                let end = motion::apply(buffer.rope(), buffer.cursor(), &Motion::LineEnd, 1);
                buffer.set_cursor_raw(end);
                buffer.replace_selection("\n");
                self.enter_insert(buffer);
                edited()
            }
            'O' if !self.mode.is_visual() => {
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
                    self.mode = Mode::Normal;
                } else {
                    self.mode = Mode::Visual;
                    buffer.set_selection(buffer.cursor(), buffer.cursor());
                }
                self.clear_pending();
                handled(false)
            }
            'V' => {
                if self.mode == Mode::VisualLine {
                    buffer.clear_selection();
                    self.mode = Mode::Normal;
                } else {
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
            // Second `g` after a pending `g` → file start (`gg`).
            'g' if self.count == usize::MAX => {
                self.count = 0;
                self.do_motion(buffer, Motion::FileStart, 1)
            }
            'g' => {
                self.count = usize::MAX;
                handled(false)
            }
            'G' => {
                if self.count == 0 || self.count == usize::MAX {
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
            'x' => self.delete_chars(buffer, count, false),
            'X' => self.delete_chars(buffer, count, true),
            'p' => self.paste(buffer, false),
            'P' => self.paste(buffer, true),
            'r' => {
                self.awaiting_replace = true;
                handled(false)
            }
            'u' => {
                let edited = buffer.undo();
                HandleResult {
                    handled: true,
                    edited,
                    ..Default::default()
                }
            }
            '.' => self.repeat_last(buffer),
            '/' => {
                self.search_draft = Some(SearchDraft {
                    pattern: String::new(),
                    forward: true,
                });
                self.clear_pending();
                handled(false)
            }
            '?' => {
                self.search_draft = Some(SearchDraft {
                    pattern: String::new(),
                    forward: false,
                });
                self.clear_pending();
                handled(false)
            }
            'n' => self.search_again(buffer, true),
            'N' => self.search_again(buffer, false),
            '*' => self.search_word(buffer),
            's' if self.mode.is_visual() => self.visual_operator(buffer, Operator::Change),
            // i/a as object prefix after operator
            'i' if self.operator.is_some() => {
                self.awaiting_object = Some(false);
                handled(false)
            }
            'a' if self.operator.is_some() => {
                self.awaiting_object = Some(true);
                handled(false)
            }
            _ => {
                if self.count == usize::MAX {
                    self.count = 0;
                }
                handled(false)
            }
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
