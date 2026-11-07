//! Exact-value setters: a size typed into a field, a theme picked from a list.

use crate::values::clamp_size;
use crate::{FontProbe, UiSettings, ensure_mono_family, ensure_ui_family};

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

    /// Pick a face for one surface. The stored name is healed against what the
    /// frontend can draw: monospace only for the editor and terminal.
    pub fn set_font_family(&mut self, surface: Surface, family: &str, probe: &dyn FontProbe) {
        match surface {
            Surface::Ui => self.ui.family = ensure_ui_family(family, probe),
            Surface::Editor => self.editor.family = ensure_mono_family(family, probe),
            Surface::Terminal => self.terminal.family = ensure_mono_family(family, probe),
        }
    }
}
