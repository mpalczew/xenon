//! Session state: sidebar chrome + content pane tree.
//! Paths in content tabs are relative to the workspace root when possible.

use serde::{Deserialize, Serialize};

use crate::content::ContentLayout;

/// Default workspace-sidebar width (px).
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.;
/// Zero-based cursor position in characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

pub fn clamp_sidebar(width: f32) -> f32 {
    width.clamp(140., 480.)
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
    #[serde(default)]
    sidebar_visible: Option<bool>,
    #[serde(default)]
    sidebar_width: Option<f32>,
    #[serde(default)]
    content: ContentLayout,
}

impl<'de> Deserialize<'de> for SessionState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let de = SessionStateDe::deserialize(deserializer)?;
        let mut content = de.content;
        content.normalize();
        Ok(SessionState {
            sidebar_visible: de.sidebar_visible.unwrap_or(true),
            sidebar_width: clamp_sidebar(de.sidebar_width.unwrap_or(DEFAULT_SIDEBAR_WIDTH)),
            content,
        })
    }
}
