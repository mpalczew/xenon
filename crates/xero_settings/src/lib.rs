//! App-wide UI settings shared across crates via gpui globals.
//! Durable values live in `~/.xero/settings.json` (via `xero_store`).
//! Also hosts clipboard actions so editor and terminal share one Cut/Copy/Paste.

use gpui::{App, Global, actions};
use xero_store::{AppSettings, DEFAULT_DARK_THEME, DEFAULT_FONT_FAMILY, DEFAULT_LIGHT_THEME};

// Shared clipboard actions (app menu, keybindings, context menus).
actions!(xero_clipboard, [Cut, Copy, Paste]);

pub use xero_store::{TerminalAutoClose, ThemeMode};

pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 32.0;
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_SHOW_LINE_NUMBERS: bool = true;
pub const DEFAULT_VIM_MODE: bool = false;

/// Font size + family for one surface (editor or terminal).
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

#[derive(Clone)]
struct EditorFont(FaceFont);
impl Global for EditorFont {}

#[derive(Clone)]
struct TerminalFont(FaceFont);
impl Global for TerminalFont {}

#[derive(Copy, Clone)]
struct ShowLineNumbers(pub bool);
impl Global for ShowLineNumbers {}

#[derive(Copy, Clone)]
struct VimMode(pub bool);
impl Global for VimMode {}

#[derive(Copy, Clone)]
struct TerminalAutoCloseSetting(pub TerminalAutoClose);
impl Global for TerminalAutoCloseSetting {}

#[derive(Clone)]
struct ThemePreference {
    mode: ThemeMode,
    light: String,
    dark: String,
}
impl Global for ThemePreference {}

/// Apply a loaded settings snapshot into gpui globals (call once at startup).
pub fn apply(settings: &AppSettings, cx: &mut App) {
    cx.set_global(EditorFont(FaceFont {
        size: clamp_size(settings.editor_font_size),
        family: settings.editor_font_family.clone(),
    }));
    cx.set_global(TerminalFont(FaceFont {
        size: clamp_size(settings.terminal_font_size),
        family: settings.terminal_font_family.clone(),
    }));
    set_show_line_numbers(cx, settings.show_line_numbers);
    set_vim_mode(cx, settings.vim_mode);
    set_terminal_auto_close(cx, settings.terminal_auto_close);
    cx.set_global(ThemePreference {
        mode: settings.theme,
        light: settings.light_theme.clone(),
        dark: settings.dark_theme.clone(),
    });
}

/// Snapshot current globals for persistence.
pub fn snapshot(cx: &App) -> AppSettings {
    let editor = editor_font(cx);
    let terminal = terminal_font(cx);
    let theme = theme_preference(cx);
    AppSettings {
        editor_font_size: editor.size,
        editor_font_family: editor.family,
        terminal_font_size: terminal.size,
        terminal_font_family: terminal.family,
        show_line_numbers: show_line_numbers(cx),
        vim_mode: vim_mode(cx),
        theme: theme.mode,
        light_theme: theme.light,
        dark_theme: theme.dark,
        terminal_auto_close: terminal_auto_close(cx),
    }
}

fn theme_preference(cx: &App) -> ThemePreference {
    cx.try_global::<ThemePreference>()
        .cloned()
        .unwrap_or_else(|| ThemePreference {
            mode: ThemeMode::System,
            light: DEFAULT_LIGHT_THEME.into(),
            dark: DEFAULT_DARK_THEME.into(),
        })
}

/// Persist current globals to disk. Logs on failure; never panics.
pub fn save(cx: &App) {
    if let Err(error) = xero_store::save_settings(&snapshot(cx)) {
        log::error!("save settings failed: {error}");
    }
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

pub fn reset_font_sizes(cx: &mut App) {
    let mut editor = editor_font(cx);
    editor.size = DEFAULT_FONT_SIZE;
    cx.set_global(EditorFont(editor));
    let mut terminal = terminal_font(cx);
    terminal.size = DEFAULT_FONT_SIZE;
    cx.set_global(TerminalFont(terminal));
}

fn clamp_size(size: f32) -> f32 {
    size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

pub fn show_line_numbers(cx: &App) -> bool {
    cx.try_global::<ShowLineNumbers>()
        .map(|setting| setting.0)
        .unwrap_or(DEFAULT_SHOW_LINE_NUMBERS)
}

fn set_show_line_numbers(cx: &mut App, show: bool) {
    cx.set_global(ShowLineNumbers(show));
}

pub fn toggle_line_numbers(cx: &mut App) {
    set_show_line_numbers(cx, !show_line_numbers(cx));
}

pub fn vim_mode(cx: &App) -> bool {
    cx.try_global::<VimMode>()
        .map(|setting| setting.0)
        .unwrap_or(DEFAULT_VIM_MODE)
}

fn set_vim_mode(cx: &mut App, enabled: bool) {
    cx.set_global(VimMode(enabled));
}

pub fn toggle_vim_mode(cx: &mut App) {
    set_vim_mode(cx, !vim_mode(cx));
}

pub fn terminal_auto_close(cx: &App) -> TerminalAutoClose {
    cx.try_global::<TerminalAutoCloseSetting>()
        .map(|setting| setting.0)
        .unwrap_or_default()
}

fn set_terminal_auto_close(cx: &mut App, mode: TerminalAutoClose) {
    cx.set_global(TerminalAutoCloseSetting(mode));
}
