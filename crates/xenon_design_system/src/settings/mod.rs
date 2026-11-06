//! GPUI side of app-wide settings: one global holding `xenon_settings::UiSettings`,
//! the font probe over GPUI's text system, and the shared clipboard actions.
//! The values and the rules that change them live in `xenon_settings`.

mod families;
mod flags;
mod prose;
mod sizes;
mod wrap;

use gpui::{App, Global, actions};
use xenon_settings::{AppSettings, UiSettings};

pub use families::{
    editor_font, ensure_mono_family, ensure_ui_family, is_monospace_family, terminal_font, ui_font,
};
pub use flags::{
    show_line_numbers, terminal_auto_close, toggle_line_numbers, toggle_vim_mode, vim_mode,
};
pub use prose::{
    nudge_prose_font_size, prose, prose_font, prose_markers, reset_prose_font_size, update_prose,
};
pub use sizes::{
    nudge_editor_font_size, nudge_terminal_font_size, nudge_ui_font_size, reset_editor_font_size,
    reset_font_sizes, reset_terminal_font_size,
};
pub use wrap::{
    editor_wraps, toggle_editor_wrap, toggle_wrap_code, toggle_wrap_prose, wrap_code, wrap_prose,
};

actions!(
    xenon_clipboard,
    [Cut, Copy, CopyClean, CopyCode, Paste, SelectAll]
);

#[derive(Clone, Default)]
struct SettingsGlobal(UiSettings);
impl Global for SettingsGlobal {}

pub(super) fn get(cx: &App) -> UiSettings {
    cx.try_global::<SettingsGlobal>()
        .map(|global| global.0.clone())
        .unwrap_or_default()
}

pub(super) fn change(cx: &mut App, edit: impl FnOnce(&mut UiSettings)) {
    let mut settings = get(cx);
    edit(&mut settings);
    cx.set_global(SettingsGlobal(settings));
}

/// Register bundled faces so editor, terminal, and UI chrome resolve.
pub fn load_embedded_fonts(cx: &mut App) {
    let fonts = xenon_settings::EMBEDDED_FONTS
        .iter()
        .map(|bytes| std::borrow::Cow::Borrowed(*bytes))
        .collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load bundled fonts: {error}");
    }
}

/// Apply a loaded settings snapshot into the global (call once at startup).
pub fn apply(settings: &AppSettings, cx: &mut App) {
    let (live, healed) = UiSettings::resolve(settings, &families::GpuiFonts(cx));
    cx.set_global(SettingsGlobal(live));
    if healed {
        save(cx);
    }
}

/// Snapshot current values for persistence, merged over what is on disk.
pub fn snapshot(cx: &App) -> AppSettings {
    get(cx).snapshot()
}

/// Persist current values to disk. Logs on failure; never panics.
pub fn save(cx: &App) {
    get(cx).persist();
}
