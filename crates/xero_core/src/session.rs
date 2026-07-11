//! Session state: what a stream shows — layout, open editors, terminal cwd.
//! All paths are relative to the stream's working directory.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Default workspace-sidebar width (px).
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.;
/// Default file-tree width (px).
pub const DEFAULT_TREE_WIDTH: f32 = 240.;
/// Default terminal pane width when split with the editor (px).
pub const DEFAULT_TERMINAL_WIDTH: f32 = 520.;

/// Which panes are visible and how wide they are in the main window.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub terminal_visible: bool,
    pub editor_visible: bool,
    pub sidebar_visible: bool,
    /// Workspace sidebar width in pixels.
    pub sidebar_width: f32,
    /// File-tree width in pixels (when the tree is open).
    pub tree_width: f32,
    /// Terminal pane width in pixels when terminal and editor are both shown.
    pub terminal_width: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            terminal_visible: true,
            editor_visible: true,
            sidebar_visible: true,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            tree_width: DEFAULT_TREE_WIDTH,
            terminal_width: DEFAULT_TERMINAL_WIDTH,
        }
    }
}

impl Layout {
    pub fn clamp_widths(mut self) -> Self {
        self.sidebar_width = clamp_sidebar(self.sidebar_width);
        self.tree_width = clamp_tree(self.tree_width);
        self.terminal_width = clamp_terminal(self.terminal_width);
        self
    }
}

pub fn clamp_sidebar(width: f32) -> f32 {
    width.clamp(140., 480.)
}

pub fn clamp_tree(width: f32) -> f32 {
    width.clamp(120., 480.)
}

pub fn clamp_terminal(width: f32) -> f32 {
    width.clamp(200., 2400.)
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
