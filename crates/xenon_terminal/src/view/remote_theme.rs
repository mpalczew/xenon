//! Which theme a phone connection shows, and its colors on the wire.

use std::sync::Arc;

use gpui::{App, Rgba};
use theme::ActiveTheme;

/// The theme a phone connection shows: `None` follows the active theme;
/// `Some(a)` keeps the active theme when it already has appearance `a`, else
/// uses the saved theme for that slot (active theme if that is unavailable).
pub fn remote_theme_for(appearance: Option<theme::Appearance>, cx: &App) -> Arc<theme::Theme> {
    let active = cx.theme().clone();
    let Some(wanted) = appearance else {
        return active;
    };
    if active.appearance == wanted {
        return active;
    }
    let settings = xenon_settings::snapshot(cx);
    let name = match wanted {
        theme::Appearance::Dark => settings.dark_theme,
        theme::Appearance::Light => settings.light_theme,
    };
    theme::ThemeRegistry::global(cx)
        .get(&name)
        .unwrap_or(active)
}

/// Terminal background, foreground, and cursor colors for the phone page.
pub fn remote_theme_colors(theme: &theme::Theme) -> [[u8; 3]; 3] {
    let colors = theme.colors();
    [
        rgb(colors.terminal_background),
        rgb(colors.terminal_foreground),
        rgb(theme.players().local().cursor),
    ]
}

pub(super) fn rgb(color: gpui::Hsla) -> [u8; 3] {
    let c: Rgba = color.into();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    [byte(c.r), byte(c.g), byte(c.b)]
}
