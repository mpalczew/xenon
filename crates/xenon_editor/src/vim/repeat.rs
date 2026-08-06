//! Last change for the `.` command.

use super::motion::Motion;
use super::object::Object;

#[derive(Clone, Debug)]
pub enum LastChange {
    /// Inserted `text` (after a change that entered insert).
    Insert { text: String },
    /// Operator applied to a motion.
    Operator {
        op: super::Operator,
        motion: Motion,
        count: usize,
        register: Option<char>,
    },
    /// Linewise double-op: `dd` / `cc` / `>>` (and count: `3dd`).
    Lines {
        op: super::Operator,
        count: usize,
        register: Option<char>,
    },
    /// Operator applied to a text object.
    Object {
        op: super::Operator,
        object: Object,
        register: Option<char>,
    },
    /// Delete one char (`x`).
    DeleteChar { count: usize },
    /// Replace char under cursor.
    Replace { ch: char },
    /// Paste from register (`p` / `P`).
    Paste { before: bool },
    /// Join lines (`J` / `gJ`). `lines` is how many lines to fuse (≥2).
    Join { lines: usize, space: bool },
    /// Increment / decrement number under cursor (`Ctrl-a` / `Ctrl-x`).
    Number { delta: i64 },
}
