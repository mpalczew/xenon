use std::fs;
use std::io::Write;

use tempfile::NamedTempFile;

use crate::{Buffer, EditCommand, ExternalState, Motion, SaveError};

fn file_with(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn open_reads_contents() {
    let file = file_with("hello\nworld\n");
    let buffer = Buffer::open(file.path()).unwrap();
    assert_eq!(buffer.text(), "hello\nworld\n");
    assert!(!buffer.is_dirty());
}

#[test]
fn open_rejects_binary() {
    let file = file_with("abc\0def");
    assert!(matches!(
        Buffer::open(file.path()),
        Err(crate::OpenError::Binary(_))
    ));
}

#[test]
fn insert_and_cursor_advances() {
    let file = file_with("");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.apply(EditCommand::Insert("fn".into()));
    assert_eq!(buffer.text(), "fn");
    assert_eq!(buffer.cursor(), 2);
    assert!(buffer.is_dirty());
}

#[test]
fn backspace_at_start_is_noop() {
    let file = file_with("x");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.apply(EditCommand::Backspace);
    assert_eq!(buffer.text(), "x");
}

#[test]
fn newline_then_up_keeps_column() {
    let file = file_with("abcd\nef");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.apply(EditCommand::SetCursor(7)); // end of "ef"
    buffer.apply(EditCommand::Move(Motion::Up));
    // Column 2 on the shorter... first line has room, so row 0 col 2.
    assert_eq!(buffer.cursor_position(), (0, 2));
}

#[test]
fn set_cursor_position_clamps_column() {
    let file = file_with("abc\nde\n");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.set_cursor_position(1, 99);
    assert_eq!(buffer.cursor_position(), (1, 2));
}

#[test]
fn set_cursor_position_clamps_row() {
    let file = file_with("abc\nde");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.set_cursor_position(99, 1);
    assert_eq!(buffer.cursor_position(), (1, 1));
}

#[test]
fn save_round_trips_bytes() {
    let file = file_with("one\ntwo\n");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.apply(EditCommand::SetCursor(0));
    buffer.apply(EditCommand::Insert("zero\n".into()));
    buffer.save().unwrap();
    assert_eq!(fs::read_to_string(file.path()).unwrap(), "zero\none\ntwo\n");
    assert!(!buffer.is_dirty());
}

#[test]
fn clean_buffer_reloads_on_external_change() {
    let file = file_with("original\n");
    let mut buffer = Buffer::open(file.path()).unwrap();
    sleep_for_mtime();
    fs::write(file.path(), "changed\n").unwrap();
    assert_eq!(buffer.check_external().unwrap(), ExternalState::Reloaded);
    assert_eq!(buffer.text(), "changed\n");
}

#[test]
fn dirty_buffer_conflicts_on_external_change() {
    let file = file_with("original\n");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.apply(EditCommand::Insert("mine ".into()));
    sleep_for_mtime();
    fs::write(file.path(), "theirs\n").unwrap();
    assert_eq!(buffer.check_external().unwrap(), ExternalState::Conflicted);
    assert!(matches!(buffer.save(), Err(SaveError::ExternalChange)));
}

#[test]
fn deleted_file_marks_dirty() {
    let file = file_with("gone\n");
    let path = file.path().to_path_buf();
    let mut buffer = Buffer::open(&path).unwrap();
    drop(file); // removes the temp file
    assert_eq!(buffer.check_external().unwrap(), ExternalState::Deleted);
    assert!(buffer.is_dirty());
}

/// mtime resolution can be coarse; nudge past it so changes are observable.
fn sleep_for_mtime() {
    std::thread::sleep(std::time::Duration::from_millis(20));
}
