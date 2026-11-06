//! Rules that change settings in place: font nudges, wrap flips, prose look.

use std::path::Path;

use crate::values::sanitize_prose;
use crate::{DEFAULT_FONT_SIZE, ProseSettings, UiSettings};

impl UiSettings {
    pub fn nudge_editor_font_size(&mut self, delta: f32) {
        self.editor.nudge(delta);
    }

    pub fn nudge_terminal_font_size(&mut self, delta: f32) {
        self.terminal.nudge(delta);
    }

    pub fn nudge_ui_font_size(&mut self, delta: f32) {
        self.ui.nudge(delta);
    }

    pub fn reset_font_sizes(&mut self) {
        self.editor.size = DEFAULT_FONT_SIZE;
        self.terminal.size = DEFAULT_FONT_SIZE;
    }

    /// Whether this file should soft-wrap. Prose defaults on; code defaults off.
    pub fn editor_wraps(&self, path: &Path) -> bool {
        if is_prose_path(path) {
            self.wrap_prose
        } else {
            self.wrap_code
        }
    }

    /// Flip wrap for the open file. No path flips the prose default.
    pub fn toggle_editor_wrap(&mut self, path: Option<&Path>) {
        if path.is_none_or(is_prose_path) {
            self.wrap_prose = !self.wrap_prose;
        } else {
            self.wrap_code = !self.wrap_code;
        }
    }

    /// Change the prose look; values are clamped to their allowed range.
    pub fn update_prose(&mut self, change: impl FnOnce(&mut ProseSettings)) {
        change(&mut self.prose);
        self.prose = sanitize_prose(self.prose.clone());
    }
}

fn is_prose_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown" | "mdx" | "txt" | "rst" | "adoc")
    )
}
