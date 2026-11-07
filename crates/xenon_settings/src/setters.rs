//! Exact-value setters: a size typed into a field, a theme picked from a list.

use crate::UiSettings;
use crate::values::clamp_size;

/// The three text surfaces with a size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Ui,
    Editor,
    Terminal,
}

impl UiSettings {
    /// Set an exact size for one surface; clamped like a nudge.
    pub fn set_font_size(&mut self, surface: Surface, size: f32) {
        let face = match surface {
            Surface::Ui => &mut self.ui,
            Surface::Editor => &mut self.editor,
            Surface::Terminal => &mut self.terminal,
        };
        face.size = clamp_size(size);
    }

    /// Store a theme name in its light or dark slot. The mode does not change.
    pub fn set_theme_name(&mut self, dark: bool, name: &str) {
        let slot = if dark {
            &mut self.theme.dark
        } else {
            &mut self.theme.light
        };
        *slot = name.to_string();
    }
}
