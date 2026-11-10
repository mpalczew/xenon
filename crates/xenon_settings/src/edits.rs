use std::path::Path;

use crate::values::sanitize_prose;
use crate::{DEFAULT_FONT_SIZE, ProseSettings, UiSettings};

impl UiSettings {
    pub fn nudge_terminal_font_size(&mut self, delta: f32) {
        self.terminal.nudge(delta);
    }

    pub fn reset_font_sizes(&mut self) {
        self.editor.size = DEFAULT_FONT_SIZE;
        self.terminal.size = DEFAULT_FONT_SIZE;
    }

    pub fn editor_wraps(&self, path: &Path) -> bool {
        if is_prose_path(path) {
            self.wrap_prose
        } else {
            self.wrap_code
        }
    }

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
