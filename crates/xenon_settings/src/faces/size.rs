//! Font size nudge / reset for editor, terminal, and UI.

use gpui::App;

use super::{EditorFont, TerminalFont, UiFont, clamp_size, editor_font, terminal_font, ui_font};
use crate::DEFAULT_FONT_SIZE;

/// Nudge editor font size by `delta` points (cmd-+ / cmd-- when editor focused).
pub fn nudge_editor_font_size(cx: &mut App, delta: f32) {
    let mut face = editor_font(cx);
    face.size = clamp_size(face.size + delta);
    cx.set_global(EditorFont(face));
}

/// Nudge terminal font size by `delta` points (cmd-+ / cmd-- when terminal focused).
pub fn nudge_terminal_font_size(cx: &mut App, delta: f32) {
    let mut face = terminal_font(cx);
    face.size = clamp_size(face.size + delta);
    cx.set_global(TerminalFont(face));
}

/// Nudge UI chrome font size (settings stepper only; not cmd-+).
pub fn nudge_ui_font_size(cx: &mut App, delta: f32) {
    let mut face = ui_font(cx);
    face.size = clamp_size(face.size + delta);
    cx.set_global(UiFont(face));
}

pub fn reset_editor_font_size(cx: &mut App) {
    let mut editor = editor_font(cx);
    editor.size = DEFAULT_FONT_SIZE;
    cx.set_global(EditorFont(editor));
}

pub fn reset_terminal_font_size(cx: &mut App) {
    let mut terminal = terminal_font(cx);
    terminal.size = DEFAULT_FONT_SIZE;
    cx.set_global(TerminalFont(terminal));
}

pub fn reset_font_sizes(cx: &mut App) {
    reset_editor_font_size(cx);
    reset_terminal_font_size(cx);
}
