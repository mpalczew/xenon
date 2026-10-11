use std::fs;
use std::io::Write;

use tempfile::NamedTempFile;

use crate::{Buffer, EditCommand, ExternalState, SaveError};

fn file_with(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn open_rejects_binary() {
    let file = file_with("abc\0def");
    assert!(matches!(
        Buffer::open(file.path()),
        Err(crate::OpenError::Binary(_))
    ));
}

/// Accidental edit then undo should clear the modified indicator.
#[test]
fn undo_to_saved_state_clears_dirty() {
    let file = file_with("hello");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.set_cursor_raw(5);
    buffer.apply(EditCommand::Insert("!".into()));
    assert!(buffer.is_dirty());
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "hello");
    assert!(!buffer.is_dirty());
    // Redo dirties again; second undo cleans again.
    assert!(buffer.redo());
    assert_eq!(buffer.text(), "hello!");
    assert!(buffer.is_dirty());
    assert!(buffer.undo());
    assert!(!buffer.is_dirty());
}

#[test]
fn undo_after_save_stays_dirty_only_past_checkpoint() {
    let file = file_with("x");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.set_cursor_raw(1);
    buffer.apply(EditCommand::Insert("y".into()));
    buffer.save().unwrap();
    assert!(!buffer.is_dirty());
    buffer.apply(EditCommand::Insert("z".into()));
    assert!(buffer.is_dirty());
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "xy");
    assert!(!buffer.is_dirty());
}

/// Change-like: delete then type, still one undo step.
#[test]
fn undo_group_delete_then_insert() {
    let file = file_with("abc");
    let mut buffer = Buffer::open(file.path()).unwrap();
    buffer.set_undo_group(true);
    buffer.set_selection(0, 3);
    buffer.delete_selection();
    buffer.apply(EditCommand::Insert("xyz".into()));
    buffer.set_undo_group(false);
    assert_eq!(buffer.text(), "xyz");
    assert!(buffer.undo());
    assert_eq!(buffer.text(), "abc");
}

/// Ungrouped edits stay separate steps (non-vim / normal commands).

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
