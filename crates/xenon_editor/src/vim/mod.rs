//! Modal editing layer. Pure buffer mutations; the view routes keys here.

mod block;
mod block_ops;
mod edits;
mod ex;
mod motion;
mod normal;
mod object;
mod ops;
mod pair;
mod paste;
mod register;
mod repeat;
mod search;
mod substitute;

#[cfg(test)]
mod count_tests;
#[cfg(test)]
mod tests;

pub use ex::{ExDraft, ExEffect};
// handle_ex_key / append_ex_char live on VimState in ex.rs
pub use motion::Motion;
pub use object::Object;
pub use register::Registers;
pub use repeat::LastChange;
pub use search::SearchState;

use crate::buffer::Buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
    /// Ctrl-v rectangular selection.
    VisualBlock,
}

impl Mode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "V-LINE",
            Mode::VisualBlock => "V-BLOCK",
        }
    }

    pub(crate) fn is_visual(self) -> bool {
        matches!(self, Mode::Visual | Mode::VisualLine | Mode::VisualBlock)
    }
}

/// Pending multi-line insert after visual-block `I` / `A`.
#[derive(Clone, Debug)]
pub(in crate::vim) struct BlockInsert {
    /// Rows that should receive the typed text (excluding the row typed into).
    other_rows: Vec<usize>,
    /// Column to insert at on each other row.
    col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
    /// `>` indent (always linewise).
    Indent,
    /// `<` outdent (always linewise).
    Outdent,
    /// `=` reindent to previous line's leading whitespace (always linewise).
    Reindent,
}

#[derive(Default)]
pub struct VimState {
    pub mode: Mode,
    count: usize,
    /// Count captured when the operator was typed; multiplied with the motion count.
    operator_count: usize,
    operator: Option<Operator>,
    awaiting_object: Option<bool>, // Some(around) after i/a following operator
    awaiting_find: Option<FindKind>,
    awaiting_replace: bool,
    register_pending: bool,
    /// Pending `g` prefix (`gg`, `gd`, `gJ`).
    pending_g: bool,
    /// Pending `z` prefix (`zz`).
    pending_z: bool,
    pub registers: Registers,
    last_find: Option<Motion>,
    pub search: Option<SearchState>,
    /// When set, the next typed chars go into the search prompt.
    pub search_draft: Option<SearchDraft>,
    /// Active `:` command line.
    pub ex_draft: Option<ExDraft>,
    /// Last `:s` replacement template (for `~`).
    last_sub_replacement: String,
    /// Brief status after ex (`3 substitutions…` / errors).
    pub ex_status: Option<String>,
    pub last_change: Option<LastChange>,
    insert_start: Option<usize>,
    insert_text: String,
    /// Anchor corner `(row, col)` while in `VisualBlock`; head is the cursor.
    block_anchor: Option<(usize, usize)>,
    /// After visual-block `I`/`A`, replicate insert text onto these rows on Esc.
    block_insert: Option<BlockInsert>,
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
    /// Ex command that needs the view/shell (write, quit, reload…).
    pub ex: Option<ExEffect>,
    pub request_definition: bool,
    /// `zz`: center the cursor line in the viewport.
    pub scroll_center: bool,
}

impl VimState {
    pub(crate) fn enter_normal(&mut self, buffer: &mut Buffer) {
        self.finish_insert_record(buffer);
        self.mode = Mode::Normal;
        self.block_anchor = None;
        self.clear_pending();
    }

    pub(crate) fn enter_insert(&mut self, buffer: &mut Buffer) {
        self.mode = Mode::Insert;
        // Group every keystroke until Esc into one undo step (classic vim / Zed).
        buffer.set_undo_group(true);
        self.insert_start = Some(buffer.cursor());
        self.insert_text.clear();
        self.clear_pending();
    }

    pub(in crate::vim) fn clear_pending(&mut self) {
        self.count = 0;
        self.operator_count = 0;
        self.operator = None;
        self.awaiting_object = None;
        self.awaiting_find = None;
        self.awaiting_replace = false;
        self.register_pending = false;
        self.pending_g = false;
        self.pending_z = false;
        let _ = self.registers.take_pending();
    }

    pub(in crate::vim) fn finish_insert_record(&mut self, buffer: &mut Buffer) {
        buffer.set_undo_group(false);
        let text = std::mem::take(&mut self.insert_text);
        if let Some(bi) = self.block_insert.take()
            && !text.is_empty()
        {
            // Replicate the typed text onto the other block rows (visual-block I/A).
            block_ops::apply_block_insert(buffer, &bi, &text);
            self.last_change = Some(LastChange::Insert { text });
        } else if !text.is_empty() {
            self.last_change = Some(LastChange::Insert { text });
        }
        self.insert_start = None;
    }

    pub(crate) fn note_insert_text(&mut self, text: &str) {
        if self.mode == Mode::Insert {
            self.insert_text.push_str(text);
        }
    }

    /// Count prefix for Ctrl-chord commands (`5Ctrl-a`); resets after read.
    pub(crate) fn count_for_ctrl(&mut self) -> usize {
        let n = self.count;
        self.count = 0;
        n
    }

    /// Handle a special key from `on_key_down` (escape, arrows, …).
    pub(crate) fn handle_key(&mut self, buffer: &mut Buffer, key: &str) -> HandleResult {
        if self.ex_draft.is_some() {
            return self.handle_ex_key(buffer, key);
        }
        if self.search_draft.is_some() {
            return self.handle_search_key(buffer, key);
        }
        match key {
            "escape" => {
                if self.mode.is_visual() {
                    buffer.clear_selection();
                }
                self.ex_status = None;
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
    pub(crate) fn handle_char(&mut self, buffer: &mut Buffer, text: &str) -> HandleResult {
        if self.ex_draft.is_some() {
            return self.append_ex_char(text);
        }
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
            LastChange::Paste { before } => self.paste(buffer, before),
            LastChange::Join { lines, space } => self.join_lines(buffer, lines, space),
            LastChange::Number { delta } => self.change_number(buffer, delta, 1),
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
