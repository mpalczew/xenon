//! Modal editing layer. Pure buffer mutations; the view routes keys here.

mod motion;
mod normal;
mod object;
mod ops;
mod pair;
mod register;
mod repeat;
mod search;

#[cfg(test)]
mod tests;

pub use motion::Motion;
pub use object::Object;
pub use register::Registers;
pub use repeat::LastChange;
pub use search::SearchState;

use crate::buffer::Buffer;
use crate::selection;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "V-LINE",
        }
    }

    pub fn is_visual(self) -> bool {
        matches!(self, Mode::Visual | Mode::VisualLine)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

#[derive(Default)]
pub struct VimState {
    pub mode: Mode,
    count: usize,
    operator: Option<Operator>,
    awaiting_object: Option<bool>, // Some(around) after i/a following operator
    awaiting_find: Option<FindKind>,
    awaiting_replace: bool,
    register_pending: bool,
    pub registers: Registers,
    last_find: Option<Motion>,
    pub search: Option<SearchState>,
    /// When set, the next typed chars go into the search prompt.
    pub search_draft: Option<SearchDraft>,
    pub last_change: Option<LastChange>,
    insert_start: Option<usize>,
    insert_text: String,
}

#[derive(Clone, Copy)]
pub(in crate::vim) enum FindKind {
    Find,
    Till,
    FindBack,
    TillBack,
}

pub struct SearchDraft {
    pub pattern: String,
    pub forward: bool,
    /// Cursor when `/` or `?` was pressed; Esc restores here.
    pub origin: usize,
    /// True when the current pattern has a match (for status + enter).
    pub has_match: bool,
}

/// Result of handling a key; view applies side effects (clipboard, notify).
#[derive(Default)]
pub struct HandleResult {
    pub handled: bool,
    pub edited: bool,
    /// Text to push to the system clipboard (for `"+` / `"*`).
    pub system_clipboard: Option<String>,
    /// Request paste from system clipboard into pending register flow.
    pub request_system_paste: bool,
}

impl VimState {
    pub fn enter_normal(&mut self, buffer: &mut Buffer) {
        self.finish_insert_record(buffer);
        self.mode = Mode::Normal;
        self.clear_pending();
    }

    pub fn enter_insert(&mut self, buffer: &mut Buffer) {
        self.mode = Mode::Insert;
        // Group every keystroke until Esc into one undo step (classic vim / Zed).
        buffer.set_undo_group(true);
        self.insert_start = Some(buffer.cursor());
        self.insert_text.clear();
        self.clear_pending();
    }

    fn clear_pending(&mut self) {
        self.count = 0;
        self.operator = None;
        self.awaiting_object = None;
        self.awaiting_find = None;
        self.awaiting_replace = false;
        self.register_pending = false;
        let _ = self.registers.take_pending();
    }

    fn finish_insert_record(&mut self, buffer: &mut Buffer) {
        buffer.set_undo_group(false);
        if !self.insert_text.is_empty() {
            self.last_change = Some(LastChange::Insert {
                text: std::mem::take(&mut self.insert_text),
            });
        }
        self.insert_start = None;
    }

    pub fn note_insert_text(&mut self, text: &str) {
        if self.mode == Mode::Insert {
            self.insert_text.push_str(text);
        }
    }

    /// Handle a special key from `on_key_down` (escape, arrows, …).
    pub fn handle_key(&mut self, buffer: &mut Buffer, key: &str) -> HandleResult {
        if self.search_draft.is_some() {
            return self.handle_search_key(buffer, key);
        }
        match key {
            "escape" => {
                if self.mode.is_visual() {
                    buffer.clear_selection();
                }
                self.enter_normal(buffer);
                return handled(false);
            }
            "backspace" if self.mode == Mode::Insert => {
                return HandleResult {
                    handled: false,
                    ..Default::default()
                };
            }
            _ => {}
        }
        if self.mode == Mode::Insert {
            return HandleResult::default();
        }
        // Arrows in normal/visual.
        if let Some(motion) = arrow_motion(key) {
            return self.do_motion(buffer, motion, 1);
        }
        HandleResult::default()
    }

    /// Handle printable input from EntityInputHandler.
    pub fn handle_char(&mut self, buffer: &mut Buffer, text: &str) -> HandleResult {
        if self.search_draft.is_some() {
            return self.append_search_char(buffer, text);
        }
        if self.mode == Mode::Insert {
            return HandleResult::default(); // view inserts
        }
        if text.chars().count() != 1 {
            // Multi-char paste-like: ignore in normal.
            return handled(false);
        }
        let ch = text.chars().next().unwrap();
        self.handle_normal_char(buffer, ch)
    }

    fn handle_search_key(&mut self, buffer: &mut Buffer, key: &str) -> HandleResult {
        match key {
            "escape" => {
                if let Some(draft) = self.search_draft.take() {
                    buffer.clear_selection();
                    buffer.set_cursor_raw(draft.origin);
                }
                handled(false)
            }
            "enter" => {
                self.commit_search(buffer);
                handled(false)
            }
            "backspace" => {
                if let Some(draft) = &mut self.search_draft {
                    draft.pattern.pop();
                }
                self.apply_incsearch(buffer);
                handled(false)
            }
            // Printable keys must NOT claim handled here: on macOS that stops
            // propagation and blocks EntityInputHandler / key_char delivery.
            _ => HandleResult::default(),
        }
    }

    /// Append text to the active `/`/`?` draft (from key_char or IME).
    fn append_search_char(&mut self, buffer: &mut Buffer, text: &str) -> HandleResult {
        if self.search_draft.is_none() {
            return HandleResult::default();
        }
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() {
            return handled(false);
        }
        if let Some(draft) = &mut self.search_draft {
            draft.pattern.push_str(&clean);
        }
        self.apply_incsearch(buffer);
        handled(false)
    }

    /// Start `/` or `?` prompt; cursor origin is restored on Esc.
    pub(in crate::vim) fn begin_search(&mut self, buffer: &Buffer, forward: bool) {
        self.search_draft = Some(SearchDraft {
            pattern: String::new(),
            forward,
            origin: buffer.cursor(),
            has_match: false,
        });
        self.clear_pending();
    }

    /// Jump to first match of the draft pattern from origin (incsearch).
    fn apply_incsearch(&mut self, buffer: &mut Buffer) {
        let Some(draft) = &self.search_draft else {
            return;
        };
        let pattern = draft.pattern.clone();
        let forward = draft.forward;
        let origin = draft.origin;
        if pattern.is_empty() {
            if let Some(d) = &mut self.search_draft {
                d.has_match = false;
            }
            buffer.clear_selection();
            buffer.set_cursor_raw(origin);
            return;
        }
        if let Some(pos) = search::find_inclusive(buffer.rope(), &pattern, origin, forward) {
            if let Some(d) = &mut self.search_draft {
                d.has_match = true;
            }
            select_match(buffer, pos, pattern.chars().count());
        } else {
            if let Some(d) = &mut self.search_draft {
                d.has_match = false;
            }
            buffer.clear_selection();
            buffer.set_cursor_raw(origin);
        }
    }

    fn commit_search(&mut self, buffer: &mut Buffer) {
        let Some(draft) = self.search_draft.take() else {
            return;
        };
        let mut pattern = draft.pattern;
        // Empty `/` reuses the last pattern (classic vim).
        if pattern.is_empty() {
            if let Some(prev) = &self.search {
                pattern = prev.pattern.clone();
            } else {
                buffer.clear_selection();
                buffer.set_cursor_raw(draft.origin);
                return;
            }
        }
        self.search = Some(SearchState::new(pattern.clone(), draft.forward));
        // Keep the incsearch landing spot when we already matched; otherwise jump.
        if draft.has_match {
            return;
        }
        if let Some(pos) =
            search::find_inclusive(buffer.rope(), &pattern, draft.origin, draft.forward)
        {
            select_match(buffer, pos, pattern.chars().count());
        } else {
            buffer.clear_selection();
            buffer.set_cursor_raw(draft.origin);
        }
    }

    fn search_again(&mut self, buffer: &mut Buffer, same_dir: bool) -> HandleResult {
        let Some(state) = &self.search else {
            return handled(false);
        };
        let pattern = state.pattern.clone();
        if pattern.is_empty() {
            return handled(false);
        }
        let forward = if same_dir {
            state.forward
        } else {
            !state.forward
        };
        let count = self.count.max(1);
        self.count = 0;
        let mut pos = None;
        for _ in 0..count {
            let from = buffer.cursor();
            pos = search::find(buffer.rope(), &pattern, from, forward);
            if let Some(p) = pos {
                // Advance cursor so the next count iteration can leave this match.
                buffer.set_cursor_raw(p);
            } else {
                break;
            }
        }
        if let Some(p) = pos {
            select_match(buffer, p, pattern.chars().count());
        }
        handled(false)
    }

    fn search_word(&mut self, buffer: &mut Buffer) -> HandleResult {
        let range = selection::word_range_at(buffer.rope(), buffer.cursor());
        let pattern = buffer.rope().slice(range).to_string();
        if pattern.is_empty() {
            return handled(false);
        }
        self.search = Some(SearchState::new(pattern.clone(), true));
        if let Some(pos) = search::find(buffer.rope(), &pattern, buffer.cursor(), true) {
            select_match(buffer, pos, pattern.chars().count());
        }
        handled(false)
    }

    fn repeat_last(&mut self, buffer: &mut Buffer) -> HandleResult {
        let Some(change) = self.last_change.clone() else {
            return handled(false);
        };
        match change {
            LastChange::Insert { text } => {
                buffer.replace_selection(&text);
                edited()
            }
            LastChange::Operator {
                op,
                motion,
                count,
                register,
            } => {
                if let Some(r) = register {
                    self.registers.set_pending(r);
                }
                let range = motion::operator_range(buffer.rope(), buffer.cursor(), &motion, count);
                self.apply_range(buffer, op, range, motion.is_linewise())
            }
            LastChange::Lines {
                op,
                count,
                register,
            } => self.repeat_lines(buffer, op, count, register),
            LastChange::Object {
                op,
                object,
                register,
            } => {
                if let Some(r) = register {
                    self.registers.set_pending(r);
                }
                if let Some(range) = object.range(buffer.rope(), buffer.cursor()) {
                    self.apply_range(buffer, op, range, false)
                } else {
                    handled(false)
                }
            }
            LastChange::DeleteChar { count } => self.delete_chars(buffer, count, false),
            LastChange::Replace { ch } => self.replace_char(buffer, ch),
            LastChange::Paste { before } => self.paste(buffer, before),
        }
    }
}

/// Highlight `[pos, pos+len)` and leave the cursor on the first match char.
fn select_match(buffer: &mut Buffer, pos: usize, len: usize) {
    let end = (pos + len).min(buffer.rope().len_chars());
    // Anchor at end, cursor at start: selection paints, cursor stays on match.
    buffer.set_selection(end, pos);
}

fn arrow_motion(key: &str) -> Option<Motion> {
    Some(match key {
        "left" => Motion::Left,
        "right" => Motion::Right,
        "up" => Motion::Up,
        "down" => Motion::Down,
        "home" => Motion::LineStart,
        "end" => Motion::LineEnd,
        _ => return None,
    })
}

pub(in crate::vim) fn handled(edited: bool) -> HandleResult {
    HandleResult {
        handled: true,
        edited,
        ..Default::default()
    }
}

pub(in crate::vim) fn edited() -> HandleResult {
    handled(true)
}
