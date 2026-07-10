//! App-wide UI settings shared across crates via gpui globals.
//! Durable values live in `~/.xero/settings.json` (via `xero_store`).
//! Also hosts clipboard actions so editor and terminal share one Cut/Copy/Paste.

use gpui::{App, Global, actions};
use xero_store::AppSettings;

// Shared clipboard actions (app menu, keybindings, context menus).
actions!(xero_clipboard, [Cut, Copy, Paste]);

pub use xero_store::ThemeMode;

pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 32.0;
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_SHOW_LINE_NUMBERS: bool = true;
pub const DEFAULT_VIM_MODE: bool = false;

/// The terminal/editor font size in points. A gpui global so every view reads
/// one source of truth and re-layouts when it changes.
#[derive(Copy, Clone)]
pub struct FontSize(pub f32);

impl Global for FontSize {}

#[derive(Copy, Clone)]
pub struct ShowLineNumbers(pub bool);

impl Global for ShowLineNumbers {}

#[derive(Copy, Clone)]
pub struct VimMode(pub bool);

impl Global for VimMode {}

#[derive(Copy, Clone)]
struct ThemePreference(ThemeMode);

impl Global for ThemePreference {}

/// Apply a loaded settings snapshot into gpui globals (call once at startup).
pub fn apply(settings: &AppSettings, cx: &mut App) {
    set_font_size(cx, settings.font_size);
    set_show_line_numbers(cx, settings.show_line_numbers);
    set_vim_mode(cx, settings.vim_mode);
    cx.set_global(ThemePreference(settings.theme));
}

/// Snapshot current globals for persistence.
pub fn snapshot(cx: &App) -> AppSettings {
    AppSettings {
        font_size: font_size(cx),
        show_line_numbers: show_line_numbers(cx),
        vim_mode: vim_mode(cx),
        theme: theme_mode(cx),
    }
}

fn theme_mode(cx: &App) -> ThemeMode {
    cx.try_global::<ThemePreference>()
        .map(|preference| preference.0)
        .unwrap_or(ThemeMode::System)
}

/// Persist current globals to disk. Logs on failure; never panics.
pub fn save(cx: &App) {
    if let Err(error) = xero_store::save_settings(&snapshot(cx)) {
        log::error!("save settings failed: {error}");
    }
}

/// The current font size (or the default if unset).
pub fn font_size(cx: &App) -> f32 {
    cx.try_global::<FontSize>()
        .map(|f| f.0)
        .unwrap_or(DEFAULT_FONT_SIZE)
}

/// Set the font size, clamped to the supported range.
pub fn set_font_size(cx: &mut App, size: f32) {
    cx.set_global(FontSize(size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)));
}

/// Nudge the font size by `delta` points (used by cmd-+/cmd--).
pub fn adjust_font_size(cx: &mut App, delta: f32) {
    let next = font_size(cx) + delta;
    set_font_size(cx, next);
}

pub fn reset_font_size(cx: &mut App) {
    set_font_size(cx, DEFAULT_FONT_SIZE);
}

pub fn show_line_numbers(cx: &App) -> bool {
    cx.try_global::<ShowLineNumbers>()
        .map(|setting| setting.0)
        .unwrap_or(DEFAULT_SHOW_LINE_NUMBERS)
}

pub fn set_show_line_numbers(cx: &mut App, show: bool) {
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

pub fn set_vim_mode(cx: &mut App, enabled: bool) {
    cx.set_global(VimMode(enabled));
}

pub fn toggle_vim_mode(cx: &mut App) {
    set_vim_mode(cx, !vim_mode(cx));
}
