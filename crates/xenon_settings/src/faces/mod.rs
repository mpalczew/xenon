//! Editor, terminal, and UI chrome font globals.

mod size;
mod ui_family;

use gpui::{App, Global};
use xenon_store::{DEFAULT_FONT_FAMILY, DEFAULT_UI_FONT_FAMILY};

use crate::{DEFAULT_FONT_SIZE, MAX_FONT_SIZE, MIN_FONT_SIZE, ensure_mono_family};

pub use size::{
    nudge_editor_font_size, nudge_terminal_font_size, nudge_ui_font_size, reset_editor_font_size,
    reset_font_sizes, reset_terminal_font_size,
};
pub use ui_family::{display_ui_family, ensure_ui_family};

/// Font size + family for one surface (editor, terminal, or UI chrome).
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFont {
    pub size: f32,
    pub family: String,
}

impl Default for FaceFont {
    fn default() -> Self {
        Self {
            size: DEFAULT_FONT_SIZE,
            family: DEFAULT_FONT_FAMILY.into(),
        }
    }
}

fn default_ui_face() -> FaceFont {
    FaceFont {
        size: DEFAULT_FONT_SIZE,
        family: DEFAULT_UI_FONT_FAMILY.into(),
    }
}

#[derive(Clone)]
pub(super) struct EditorFont(pub(super) FaceFont);
impl Global for EditorFont {}

#[derive(Clone)]
pub(super) struct TerminalFont(pub(super) FaceFont);
impl Global for TerminalFont {}

#[derive(Clone)]
pub(super) struct UiFont(pub(super) FaceFont);
impl Global for UiFont {}

/// Install resolved faces.
pub(crate) fn set_faces(editor: FaceFont, terminal: FaceFont, ui: FaceFont, cx: &mut App) {
    cx.set_global(EditorFont(FaceFont {
        size: clamp_size(editor.size),
        family: editor.family,
    }));
    cx.set_global(TerminalFont(FaceFont {
        size: clamp_size(terminal.size),
        family: terminal.family,
    }));
    cx.set_global(UiFont(FaceFont {
        size: clamp_size(ui.size),
        family: ui.family,
    }));
}

/// Unresolved face triple from settings.json (family not yet healed).
pub(crate) struct RawFaces<'a> {
    pub editor: (&'a str, f32),
    pub terminal: (&'a str, f32),
    pub ui: (&'a str, f32),
}

/// Resolve editor/terminal (mono) and UI faces from raw settings fields.
pub(crate) fn resolve_faces(raw: RawFaces<'_>, cx: &App) -> (FaceFont, FaceFont, FaceFont, bool) {
    let (editor_family, editor_size) = raw.editor;
    let (terminal_family, terminal_size) = raw.terminal;
    let (ui_family, ui_size) = raw.ui;
    let editor_resolved = ensure_mono_family(editor_family, cx);
    let terminal_resolved = ensure_mono_family(terminal_family, cx);
    let ui_resolved = ensure_ui_family(ui_family, cx);
    let healed = editor_resolved != editor_family
        || terminal_resolved != terminal_family
        || ui_resolved != ui_family;
    (
        FaceFont {
            size: editor_size,
            family: editor_resolved,
        },
        FaceFont {
            size: terminal_size,
            family: terminal_resolved,
        },
        FaceFont {
            size: ui_size,
            family: ui_resolved,
        },
        healed,
    )
}

pub fn editor_font(cx: &App) -> FaceFont {
    cx.try_global::<EditorFont>()
        .map(|f| f.0.clone())
        .unwrap_or_default()
}

pub fn terminal_font(cx: &App) -> FaceFont {
    cx.try_global::<TerminalFont>()
        .map(|f| f.0.clone())
        .unwrap_or_default()
}

pub fn ui_font(cx: &App) -> FaceFont {
    cx.try_global::<UiFont>()
        .map(|f| f.0.clone())
        .unwrap_or_else(default_ui_face)
}

pub(super) fn clamp_size(size: f32) -> f32 {
    size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}
