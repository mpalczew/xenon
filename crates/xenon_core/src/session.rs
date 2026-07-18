//! Session state: sidebar chrome + content pane tree.
//! Paths in content tabs are relative to the workspace root when possible.

use serde::{Deserialize, Serialize};

use crate::content::{ContentLayout, LegacySession, migrate_from_legacy};

/// Default workspace-sidebar width (px).
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.;
/// Legacy default terminal column width (migration only).
pub const DEFAULT_TERMINAL_WIDTH: f32 = 520.;

/// Zero-based cursor position in characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

/// Legacy editor row (migration / tests).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenEditor {
    pub path: std::path::PathBuf,
    #[serde(default)]
    pub cursor: Point,
    #[serde(default)]
    pub scroll_top: u32,
}

/// Legacy terminal cwd hint (migration / tests).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalState {
    pub cwd: std::path::PathBuf,
}

impl Default for TerminalState {
    fn default() -> Self {
        TerminalState {
            cwd: std::path::PathBuf::from("."),
        }
    }
}

/// Legacy layout flags (migration / tests only).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub terminal_visible: bool,
    pub editor_visible: bool,
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
    pub terminal_width: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            terminal_visible: true,
            editor_visible: true,
            sidebar_visible: true,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            terminal_width: DEFAULT_TERMINAL_WIDTH,
        }
    }
}

impl Layout {
    pub fn clamp_widths(mut self) -> Self {
        self.sidebar_width = clamp_sidebar(self.sidebar_width);
        self.terminal_width = clamp_terminal(self.terminal_width);
        self
    }
}

pub fn clamp_sidebar(width: f32) -> f32 {
    width.clamp(140., 480.)
}

pub fn clamp_terminal(width: f32) -> f32 {
    width.clamp(200., 2400.)
}

/// Persisted per-workspace session.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessionState {
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
    pub content: ContentLayout,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState {
            sidebar_visible: true,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            content: ContentLayout::default(),
        }
    }
}

impl SessionState {
    pub fn clamp_widths(mut self) -> Self {
        self.sidebar_width = clamp_sidebar(self.sidebar_width);
        self.content.normalize();
        self
    }
}

#[derive(Deserialize)]
struct SessionStateDe {
    // New fields
    #[serde(default)]
    sidebar_visible: Option<bool>,
    #[serde(default)]
    sidebar_width: Option<f32>,
    #[serde(default)]
    content: Option<ContentLayout>,
    // Nested new-style optional (if someone nested under layout — ignore)
    // Legacy flat layout
    #[serde(default)]
    layout: Option<Layout>,
    #[serde(default)]
    editors: Vec<OpenEditor>,
    #[serde(default)]
    active_editor: Option<usize>,
    #[serde(default)]
    terminal: Option<TerminalState>,
    // Legacy flat keys (old Layout without nesting)
    #[serde(default)]
    terminal_visible: Option<bool>,
    #[serde(default)]
    editor_visible: Option<bool>,
    #[serde(default)]
    terminal_width: Option<f32>,
}

impl<'de> Deserialize<'de> for SessionState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let de = SessionStateDe::deserialize(deserializer)?;
        if let Some(mut content) = de.content {
            content.normalize();
            let sidebar_visible = de
                .sidebar_visible
                .or_else(|| de.layout.map(|l| l.sidebar_visible))
                .unwrap_or(true);
            let sidebar_width = de
                .sidebar_width
                .or_else(|| de.layout.map(|l| l.sidebar_width))
                .unwrap_or(DEFAULT_SIDEBAR_WIDTH);
            return Ok(SessionState {
                sidebar_visible,
                sidebar_width: clamp_sidebar(sidebar_width),
                content,
            });
        }

        // Legacy session
        let layout = de.layout.unwrap_or_else(|| Layout {
            terminal_visible: de.terminal_visible.unwrap_or(true),
            editor_visible: de.editor_visible.unwrap_or(true),
            sidebar_visible: de.sidebar_visible.unwrap_or(true),
            sidebar_width: de.sidebar_width.unwrap_or(DEFAULT_SIDEBAR_WIDTH),
            terminal_width: de.terminal_width.unwrap_or(DEFAULT_TERMINAL_WIDTH),
        });
        let layout = layout.clamp_widths();
        let content = migrate_from_legacy(LegacySession {
            terminal_visible: layout.terminal_visible,
            editor_visible: layout.editor_visible,
            terminal_width: layout.terminal_width,
            editors: de.editors,
            active_editor: de.active_editor,
            terminal: de.terminal.unwrap_or_default(),
        });
        Ok(SessionState {
            sidebar_visible: layout.sidebar_visible,
            sidebar_width: layout.sidebar_width,
            content,
        })
    }
}
