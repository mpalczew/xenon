//! Integration tests for vim motions, operators, paste, and `.`.

use std::io::Write;

use ropey::Rope;
use tempfile::NamedTempFile;

use super::motion::{self, Motion};
use super::repeat::LastChange;
use super::{Mode, VimState};
use crate::buffer::Buffer;

fn buffer_with(text: &str) -> Buffer {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.flush().unwrap();
    Buffer::open(file.path()).unwrap()
}

#[test]
fn word_forward_skips_spaces() {
    let rope = Rope::from_str("foo bar");
    assert_eq!(motion::apply(&rope, 0, &Motion::WordForward, 1), 4);
}

#[test]
fn right_stops_at_line_end() {
    let rope = Rope::from_str("ab\ncd");
    // On 'b' (1); further l stays put, never lands on next line.
    assert_eq!(motion::apply(&rope, 1, &Motion::Right, 1), 1);
    assert_eq!(motion::apply(&rope, 1, &Motion::Right, 5), 1);
}

#[test]
fn dollar_lands_on_last_content_char() {
    let rope = Rope::from_str("ab\ncd\n");
    // `$` → 'b' (1), not the newline (2).
    assert_eq!(motion::apply(&rope, 0, &Motion::LineEnd, 1), 1);
    assert_eq!(motion::apply(&rope, 3, &Motion::LineEnd, 1), 4);
}

#[test]
fn d_dollar_excludes_newline() {
    let rope = Rope::from_str("hello\nworld\n");
    // From 'e' (1) through end of content only.
    assert_eq!(motion::operator_range(&rope, 1, &Motion::LineEnd, 1), 1..5);
    // Empty line: no-op range (do not eat the newline).
    let empty = Rope::from_str("\nnext\n");
    assert_eq!(motion::operator_range(&empty, 0, &Motion::LineEnd, 1), 0..0);
}

#[test]
fn left_stops_at_line_start() {
    let rope = Rope::from_str("ab\ncd");
    // On 'c' (3); h does not wrap to previous line.
    assert_eq!(motion::apply(&rope, 3, &Motion::Left, 1), 3);
    assert_eq!(motion::apply(&rope, 3, &Motion::Left, 5), 3);
}

#[test]
fn right_moves_within_line() {
    let rope = Rope::from_str("abcd\n");
    assert_eq!(motion::apply(&rope, 0, &Motion::Right, 2), 2);
    assert_eq!(motion::apply(&rope, 0, &Motion::Right, 10), 3);
}

#[test]
fn find_forward() {
    let rope = Rope::from_str("abxcd");
    assert_eq!(
        motion::apply(
            &rope,
            0,
            &Motion::Find {
                ch: 'x',
                before: false,
                forward: true
            },
            1
        ),
        2
    );
}

#[test]
fn dd_then_p_pastes_below_current_line() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    // Cursor on "aaa", delete the line.
    assert!(vim.handle_char(&mut buffer, "d").handled);
    assert!(vim.handle_char(&mut buffer, "d").edited);
    assert_eq!(buffer.text(), "bbb\nccc\n");
    // Move to "ccc" (second remaining line).
    vim.handle_char(&mut buffer, "j");
    // Paste linewise below.
    assert!(vim.handle_char(&mut buffer, "p").edited);
    assert_eq!(buffer.text(), "bbb\nccc\naaa\n");
}

#[test]
fn dd_then_big_p_pastes_above_current_line() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, "d");
    vim.handle_char(&mut buffer, "d");
    // On "bbb", paste above.
    assert!(vim.handle_char(&mut buffer, "P").edited);
    assert_eq!(buffer.text(), "aaa\nbbb\nccc\n");
}

#[test]
fn charwise_delete_still_pastes_inline() {
    let mut buffer = buffer_with("ab\ncd\n");
    let mut vim = VimState::default();
    // `x` is characterwise.
    vim.handle_char(&mut buffer, "x");
    assert_eq!(buffer.text(), "b\ncd\n");
    vim.handle_char(&mut buffer, "j");
    vim.handle_char(&mut buffer, "p");
    // Paste after the char under the cursor on "cd".
    assert_eq!(buffer.text(), "b\ncad\n");
}

#[test]
fn d_dollar_does_not_join_lines() {
    let mut buffer = buffer_with("hello\nworld\n");
    let mut vim = VimState::default();
    // On 'h', d$ deletes "hello" only.
    vim.handle_char(&mut buffer, "d");
    vim.handle_char(&mut buffer, "$");
    assert_eq!(buffer.text(), "\nworld\n");
}

#[test]
fn a_at_eol_stays_on_same_line() {
    let mut buffer = buffer_with("ab\ncd\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, "$");
    // Cursor on last content char; `a` inserts before the newline.
    vim.handle_char(&mut buffer, "a");
    assert_eq!(vim.mode, Mode::Insert);
    // Insert position is exclusive EOL of first line (index 2).
    assert_eq!(buffer.cursor(), 2);
    buffer.replace_selection("X");
    assert_eq!(buffer.text(), "abX\ncd\n");
}

#[test]
fn dot_repeats_dd() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    // Prior insert must not win over dd for `.`.
    let mut vim = VimState {
        last_change: Some(LastChange::Insert {
            text: "NOPE\n".into(),
        }),
        ..Default::default()
    };
    vim.handle_char(&mut buffer, "d");
    vim.handle_char(&mut buffer, "d");
    assert_eq!(buffer.text(), "bbb\nccc\n");
    vim.handle_char(&mut buffer, ".");
    assert_eq!(buffer.text(), "ccc\n");
}

#[test]
fn visual_x_deletes_selection() {
    let mut buffer = buffer_with("abcdef\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, "v");
    vim.handle_char(&mut buffer, "l");
    vim.handle_char(&mut buffer, "l");
    assert_eq!(vim.mode, Mode::Visual);
    assert!(vim.handle_char(&mut buffer, "x").edited);
    assert_eq!(buffer.text(), "def\n");
    assert_eq!(vim.mode, Mode::Normal);
}

#[test]
fn visual_line_d_deletes_lines() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, "V");
    vim.handle_char(&mut buffer, "j");
    assert!(vim.handle_char(&mut buffer, "d").edited);
    assert_eq!(buffer.text(), "ccc\n");
}

#[test]
fn colon_opens_ex_draft() {
    let mut buffer = buffer_with("hi\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, ":");
    assert!(vim.ex_draft.is_some());
    vim.handle_char(&mut buffer, "w");
    assert_eq!(vim.ex_draft.as_ref().unwrap().line, "w");
}

#[test]
fn visual_colon_prefills_range() {
    let mut buffer = buffer_with("a\nb\nc\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, "V");
    vim.handle_char(&mut buffer, "j");
    vim.handle_char(&mut buffer, ":");
    let draft = vim.ex_draft.as_ref().unwrap();
    assert_eq!(draft.line, "'<,'>");
    assert_eq!(draft.visual_lines, Some((0, 1)));
}
