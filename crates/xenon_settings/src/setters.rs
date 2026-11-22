use crate::UiSettings;
use crate::values::clamp_size;

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

    pub fn set_font_family(&mut self, surface: Surface, family: &str) {
        let face = match surface {
            Surface::Ui => &mut self.ui,
            Surface::Editor => &mut self.editor,
            Surface::Terminal => &mut self.terminal,
        };
        face.family = family.trim().to_string();
    }
}
