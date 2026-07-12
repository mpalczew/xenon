//! Modal editing layer. Pure buffer mutations; the view routes keys here.

mod motion;
mod normal;
mod object;
mod ops;
mod register;
mod repeat;
mod search;

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
        if let Some(draft) = &mut self.search_draft {
            draft.pattern.push_str(text);
            return handled(false);
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
                self.search_draft = None;
                handled(false)
            }
            "enter" => {
                if let Some(draft) = self.search_draft.take() {
                    self.search = Some(SearchState::new(draft.pattern.clone(), draft.forward));
                    if let Some(pos) = search::find(
                        buffer.rope(),
                        &draft.pattern,
                        buffer.cursor(),
                        draft.forward,
                    ) {
                        buffer.clear_selection();
                        buffer.set_cursor_raw(pos);
                    }
                }
                handled(false)
            }
            "backspace" => {
                if let Some(draft) = &mut self.search_draft {
                    draft.pattern.pop();
                }
                handled(false)
            }
            _ => handled(false),
        }
    }

    fn search_again(&mut self, buffer: &mut Buffer, same_dir: bool) -> HandleResult {
        let Some(state) = &self.search else {
            return handled(false);
        };
        let forward = if same_dir {
            state.forward
        } else {
            !state.forward
        };
        if let Some(pos) = search::find(buffer.rope(), &state.pattern, buffer.cursor(), forward) {
            buffer.clear_selection();
            buffer.set_cursor_raw(pos);
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
            buffer.clear_selection();
            buffer.set_cursor_raw(pos);
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
        }
    }
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
