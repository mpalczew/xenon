//! Counted operators: `2dd`, count multiply.

use std::io::Write;

use tempfile::NamedTempFile;

use super::VimState;
use crate::buffer::Buffer;

fn buffer_with(text: &str) -> Buffer {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.flush().unwrap();
    Buffer::open(file.path()).unwrap()
}

fn type_keys(vim: &mut VimState, buffer: &mut Buffer, keys: &str) {
    for ch in keys.chars() {
        vim.handle_char(buffer, &ch.to_string());
    }
}

#[test]
fn two_dd_then_p_pastes_both_lines() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "2dd");
    assert_eq!(buffer.text(), "ccc\n");
    assert!(vim.handle_char(&mut buffer, "p").edited);
    assert_eq!(buffer.text(), "ccc\naaa\nbbb\n");
}

#[test]
fn d_two_d_deletes_two_lines() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "d2d");
    assert_eq!(buffer.text(), "ccc\n");
}

#[test]
fn two_d_three_d_multiplies_counts() {
    let mut buffer = buffer_with("a\nb\nc\nd\ne\nf\ng\nh\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "2d3d");
    assert_eq!(buffer.text(), "g\nh\n");
}

#[test]
fn dot_repeats_two_dd() {
    let mut buffer = buffer_with("a\nb\nc\nd\ne\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "2dd");
    assert_eq!(buffer.text(), "c\nd\ne\n");
    vim.handle_char(&mut buffer, ".");
    assert_eq!(buffer.text(), "e\n");
}
