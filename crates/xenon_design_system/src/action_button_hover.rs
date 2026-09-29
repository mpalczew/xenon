//! Hover colors for `ActionButton`: the button owns its single `.hover`
//! style (GPUI asserts on a second), so callers configure it here.

use gpui::Hsla;

use crate::ActionButton;

impl<L> ActionButton<L> {
    /// Text color on hover.
    pub fn hover_text(mut self, color: Hsla) -> Self {
        self.hover_text = Some(color);
        self
    }

    /// Background on hover, replacing the variant's default.
    pub fn hover_background(mut self, color: Hsla) -> Self {
        self.hover_background = Some(color);
        self
    }
}
