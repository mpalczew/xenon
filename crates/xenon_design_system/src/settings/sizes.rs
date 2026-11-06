//! Font size nudges and resets.

use gpui::App;
use xenon_settings::UiSettings;

use super::change;

/// Nudge editor font size by `delta` points (cmd-+ / cmd-- when editor focused).
pub fn nudge_editor_font_size(cx: &mut App, delta: f32) {
    change(cx, |s| s.nudge_editor_font_size(delta));
}

/// Nudge terminal font size by `delta` points (cmd-+ / cmd-- when terminal focused).
pub fn nudge_terminal_font_size(cx: &mut App, delta: f32) {
    change(cx, |s| s.nudge_terminal_font_size(delta));
}

/// Nudge UI chrome font size (settings stepper only; not cmd-+).
pub fn nudge_ui_font_size(cx: &mut App, delta: f32) {
    change(cx, |s| s.nudge_ui_font_size(delta));
}

pub fn reset_editor_font_size(cx: &mut App) {
    change(cx, |s| s.editor.size = xenon_settings::DEFAULT_FONT_SIZE);
}

pub fn reset_terminal_font_size(cx: &mut App) {
    change(cx, |s| s.terminal.size = xenon_settings::DEFAULT_FONT_SIZE);
}

pub fn reset_font_sizes(cx: &mut App) {
    change(cx, UiSettings::reset_font_sizes);
}
