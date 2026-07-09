//! App-wide UI settings shared across crates via gpui globals.

use gpui::{App, Global};

pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 32.0;
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_SHOW_LINE_NUMBERS: bool = true;

/// The terminal/editor font size in points. A gpui global so every view reads
/// one source of truth and re-layouts when it changes.
#[derive(Copy, Clone)]
pub struct FontSize(pub f32);

impl Global for FontSize {}

#[derive(Copy, Clone)]
pub struct ShowLineNumbers(pub bool);

impl Global for ShowLineNumbers {}

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
