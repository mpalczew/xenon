//! Session state: what a stream shows — layout, open editors, terminal cwd.
//! All paths are relative to the stream's working directory.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How the main panel is arranged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Layout {
    TerminalOnly,
    EditorOnly,
    Split { ratio: f32 },
}

impl Default for Layout {
    fn default() -> Self {
        Layout::TerminalOnly
    }
}

impl Layout {
    /// Clamp a split ratio to a sane visible range.
    pub const MIN_RATIO: f32 = 0.15;
    pub const MAX_RATIO: f32 = 0.85;

    pub fn split(ratio: f32) -> Self {
        Layout::Split { ratio: ratio.clamp(Self::MIN_RATIO, Self::MAX_RATIO) }
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
        TerminalState { cwd: PathBuf::from(".") }
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
