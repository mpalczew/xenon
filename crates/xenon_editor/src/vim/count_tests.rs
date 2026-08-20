//! Counted operators: `2dd`, count multiply, nvim oracle.

use std::io::Write;

use ropey::Rope;
use tempfile::NamedTempFile;

use super::VimState;
use super::motion::{self, Motion};
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
fn line_motion_range_covers_count_lines() {
    let rope = Rope::from_str("aaa\nbbb\nccc\n");
    assert_eq!(motion::operator_range(&rope, 0, &Motion::Line, 1), 0..4);
    assert_eq!(motion::operator_range(&rope, 0, &Motion::Line, 2), 0..8);
    assert_eq!(motion::operator_range(&rope, 0, &Motion::Line, 3), 0..12);
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
fn two_dj_deletes_three_lines() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\nddd\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "2dj");
    assert_eq!(buffer.text(), "ddd\n");
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
fn two_yy_then_p_pastes_two_lines() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "2yyp");
    // Linewise `p` inserts below the current line, so original `bbb` stays.
    assert_eq!(buffer.text(), "aaa\naaa\nbbb\nbbb\nccc\n");
}

#[test]
fn three_dw_still_deletes_three_words() {
    let mut buffer = buffer_with("foo bar baz qux\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "3dw");
    assert_eq!(buffer.text(), "qux\n");
}

#[test]
fn two_d_three_w_multiplies() {
    let mut buffer = buffer_with("a b c d e f g\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "2d3w");
    assert_eq!(buffer.text(), "g\n");
}

#[test]
fn two_y_shortcut_yanks_two_lines() {
    let mut buffer = buffer_with("aaa\nbbb\nccc\n");
    let mut vim = VimState::default();
    type_keys(&mut vim, &mut buffer, "2Yp");
    assert_eq!(buffer.text(), "aaa\naaa\nbbb\nbbb\nccc\n");
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

/// Headless Neovim is the answer key, not the runtime. Skip if `nvim` is missing.
fn nvim_normal(text: &str, keys: &str) -> Option<String> {
    let mut file = NamedTempFile::new().ok()?;
    file.write_all(text.as_bytes()).ok()?;
    file.flush().ok()?;
    let path = file.path();
    let status = std::process::Command::new("nvim")
        .args([
            "--headless",
            "-u",
            "NONE",
            "-n",
            "-c",
            &format!("normal! {keys}"),
            "-c",
            "wq",
        ])
        .arg(path)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

#[test]
fn counted_linewise_matches_nvim_when_present() {
    let cases = [
        ("aaa\nbbb\nccc\n", "2ddp"),
        ("aaa\nbbb\nccc\n", "2yyp"),
        ("a\nb\nc\nd\ne\nf\ng\nh\n", "2d3d"),
        ("aaa\nbbb\nccc\nddd\n", "2dj"),
    ];
    let mut ran = 0;
    for (text, keys) in cases {
        let Some(want) = nvim_normal(text, keys) else {
            continue;
        };
        ran += 1;
        let mut buffer = buffer_with(text);
        let mut vim = VimState::default();
        type_keys(&mut vim, &mut buffer, keys);
        assert_eq!(buffer.text(), want, "keys {keys:?}");
    }
    if ran == 0 {
        eprintln!("skipping nvim oracle: nvim not available");
    }
}
