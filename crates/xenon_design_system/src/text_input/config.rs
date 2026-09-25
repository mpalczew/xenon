use gpui::{Pixels, px};

pub struct TextInputConfig {
    pub placeholder: String,
    pub multiline: bool,
    pub min_height: Pixels,
    pub appearance: TextInputAppearance,
    pub key_behavior: TextInputKeyBehavior,
}

#[derive(Clone, Copy)]
pub enum TextInputAppearance {
    Bordered,
    Inline,
    Palette,
}

#[derive(Clone, Copy)]
pub enum TextInputKeyBehavior {
    SubmitAndCancel,
    SubmitOnPlainEnter,
    ParentNavigation,
    DialogField,
}

impl TextInputConfig {
    pub fn single_line(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
            multiline: false,
            min_height: px(28.),
            appearance: TextInputAppearance::Inline,
            key_behavior: TextInputKeyBehavior::SubmitAndCancel,
        }
    }

    pub fn multiline(placeholder: impl Into<String>, min_height: Pixels) -> Self {
        Self {
            placeholder: placeholder.into(),
            multiline: true,
            min_height,
            appearance: TextInputAppearance::Bordered,
            key_behavior: TextInputKeyBehavior::SubmitAndCancel,
        }
    }

    pub fn appearance(mut self, appearance: TextInputAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn parent_navigation(mut self) -> Self {
        self.key_behavior = TextInputKeyBehavior::ParentNavigation;
        self
    }

    pub fn dialog_field(mut self) -> Self {
        self.key_behavior = TextInputKeyBehavior::DialogField;
        self
    }

    pub fn submit_on_plain_enter(mut self) -> Self {
        self.key_behavior = TextInputKeyBehavior::SubmitOnPlainEnter;
        self
    }
}
