//! Rules for where and how an opened file is shown.

use crate::ipc::{OpenFileSpec, OpenPane};

impl OpenPane {
    /// `--pane focused` is the one policy that should take editor focus.
    pub fn takes_editor_focus(self) -> bool {
        matches!(self, Self::Focused)
    }
}

impl OpenFileSpec {
    /// Zero-based (row, column) of the 1-based `line`/`column`, if a line is given.
    pub fn cursor(&self) -> Option<(u32, u32)> {
        self.line.map(|line| {
            (
                line.saturating_sub(1),
                self.column.unwrap_or(1).saturating_sub(1),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_focused_pane_steals_editor_focus() {
        assert!(OpenPane::Focused.takes_editor_focus());
        assert!(!OpenPane::Sibling.takes_editor_focus());
        assert!(!OpenPane::SplitRight.takes_editor_focus());
    }
}
