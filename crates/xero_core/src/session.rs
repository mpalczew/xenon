//! Session state: what a stream shows — layout, open editors, terminal cwd.
//! All paths are relative to the stream's working directory.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Which panes are visible in the main window.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub terminal_visible: bool,
    pub editor_visible: bool,
    pub sidebar_visible: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            terminal_visible: true,
            editor_visible: true,
            sidebar_visible: true,
        }
    }
}

/// A zero-based cursor position in characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

/// An editor open in the session; `path` is relative to the working directory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenEditor {
    pub path: PathBuf,
    #[serde(default)]
    pub cursor: Point,
    #[serde(default)]
    pub scroll_top: u32,
}

/// Terminal restore hint; a fresh shell is spawned at `cwd` (relative).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalState {
    pub cwd: PathBuf,
}

impl Default for TerminalState {
    fn default() -> Self {
        TerminalState {
            cwd: PathBuf::from("."),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub layout: Layout,
    #[serde(default)]
    pub editors: Vec<OpenEditor>,
    #[serde(default)]
    pub active_editor: Option<usize>,
    #[serde(default)]
    pub terminal: TerminalState,
}
