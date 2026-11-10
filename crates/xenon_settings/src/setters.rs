use crate::values::clamp_size;
use crate::{FontProbe, UiSettings, ensure_mono_family, ensure_ui_family};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Ui,
    Editor,
    Terminal,
}

impl UiSettings {
    pub fn set_font_size(&mut self, surface: Surface, size: f32) {
        let face = match surface {
            Surface::Ui => &mut self.ui,
            Surface::Editor => &mut self.editor,
            Surface::Terminal => &mut self.terminal,
        };
        face.size = clamp_size(size);
    }

    pub fn set_font_family(&mut self, surface: Surface, family: &str, probe: &dyn FontProbe) {
        match surface {
            Surface::Ui => self.ui.family = ensure_ui_family(family, probe),
            Surface::Editor => self.editor.family = ensure_mono_family(family, probe),
            Surface::Terminal => self.terminal.family = ensure_mono_family(family, probe),
        }
    }
}
