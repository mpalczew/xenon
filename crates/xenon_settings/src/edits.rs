use std::path::Path;

use crate::values::sanitize_prose;
use crate::{DEFAULT_FONT_SIZE, ProseSettings, Surface, UiSettings};

impl UiSettings {
    pub fn zoom(&mut self, surface: Surface, step: Option<f32>) {
        let current = match surface {
            Surface::Ui => self.ui.size,
            Surface::Editor => self.editor.size,
            Surface::Terminal => self.terminal.size,
        };
        let size = step.map_or(DEFAULT_FONT_SIZE, |d| current + d);
        self.set_font_size(surface, size);
    }

    pub fn zoom_prose(&mut self, step: Option<f32>) {
        self.update_prose(|p| {
            p.size = step.map_or(ProseSettings::default().size, |d| p.size + d);
        });
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
