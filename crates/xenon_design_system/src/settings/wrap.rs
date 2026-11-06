//! Soft wrap defaults.

use std::path::Path;

use gpui::App;

use super::{change, get};

pub fn editor_wraps(path: &Path, cx: &App) -> bool {
    get(cx).editor_wraps(path)
}

pub fn toggle_editor_wrap(path: Option<&Path>, cx: &mut App) {
    change(cx, |s| s.toggle_editor_wrap(path));
}

/// Soft wrap default for Markdown and other prose.
pub fn wrap_prose(cx: &App) -> bool {
    get(cx).wrap_prose
}

/// Soft wrap default for every other text file.
pub fn wrap_code(cx: &App) -> bool {
    get(cx).wrap_code
}

pub fn toggle_wrap_prose(cx: &mut App) {
    change(cx, |s| s.wrap_prose = !s.wrap_prose);
}

pub fn toggle_wrap_code(cx: &mut App) {
    change(cx, |s| s.wrap_code = !s.wrap_code);
}
