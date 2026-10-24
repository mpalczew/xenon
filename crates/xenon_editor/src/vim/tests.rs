//! Integration tests for vim motions, operators, paste, and `.`.

use std::io::Write;

use ropey::Rope;
use tempfile::NamedTempFile;

use super::motion::{self, Motion};
use super::{Mode, VimState};
use crate::buffer::Buffer;

fn buffer_with(text: &str) -> Buffer {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.flush().unwrap();
    Buffer::open(file.path()).unwrap()
}

#[test]
fn d_dollar_excludes_newline() {
    let rope = Rope::from_str("hello\nworld\n");
    assert_eq!(motion::operator_range(&rope, 1, &Motion::LineEnd, 1), 1..5);
    let empty = Rope::from_str("\nnext\n");
    assert_eq!(motion::operator_range(&empty, 0, &Motion::LineEnd, 1), 0..0);
}

#[test]
fn dd_then_p_pastes_below_current_line() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    assert!(vim.handle_char(&mut buffer, "d").handled);
    assert!(vim.handle_char(&mut buffer, "d").edited);
    assert_eq!(buffer.text(), "bbb\nccc\n");
    vim.handle_char(&mut buffer, "j");
    assert!(vim.handle_char(&mut buffer, "p").edited);
    assert_eq!(buffer.text(), "bbb\nccc\naaa\n");
}

#[test]
fn d_dollar_does_not_join_lines() {
    let mut buffer = buffer_with("hello\nworld\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, "d");
    vim.handle_char(&mut buffer, "$");
    assert_eq!(buffer.text(), "\nworld\n");
}

#[test]
fn a_at_eol_stays_on_same_line() {
    let mut buffer = buffer_with("ab\ncd\n");
    let mut vim = VimState::default();
    vim.handle_char(&mut buffer, "$");
    vim.handle_char(&mut buffer, "a");
    assert_eq!(vim.mode, Mode::Insert);
    assert_eq!(buffer.cursor(), 2);
    buffer.replace_selection("X");
    assert_eq!(buffer.text(), "abX\ncd\n");
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
