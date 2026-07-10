//! One-time initialization of the zed globals that `terminal` depends on.
//!
//! `terminal::TerminalBuilder` reads `release_channel::AppVersion`,
//! `TerminalSettings`, and font settings from `ThemeSettings`, all of which are
//! gpui globals. They must be installed before the first terminal spawns.

use gpui::{App, Subscription, Window};
use settings::Settings;
use xero_settings::ThemeMode;

/// zed's built-in One theme family (One Light + One Dark), vendored from
/// zed's `assets/themes/one/one.json`. See ATTRIBUTION.md.
const ONE_THEME: &[u8] = include_bytes!("../assets/one.json");

/// Install every global the terminal backend needs. Call once, at startup,
/// inside `application().run(|cx| ...)`.
pub fn init(cx: &mut App) {
    settings::init(cx);
    release_channel::init(app_version(), cx);
    theme::init(theme::LoadThemes::JustBase, cx);
    theme_settings::ThemeSettings::register(cx);
    terminal::terminal_settings::TerminalSettings::register(cx);
    load_themes(cx);
}

/// Load One Light + One Dark, then select per the current theme preference.
fn load_themes(cx: &mut App) {
    let content = match theme_settings::deserialize_user_theme(ONE_THEME) {
        Ok(content) => content,
        Err(error) => {
            log::error!("failed to parse bundled theme: {error}");
            return;
        }
    };
    let family = theme_settings::refine_theme_family(content);
    theme::ThemeRegistry::global(cx).insert_theme_families([family]);
    apply_theme(cx);
}

/// Point the global theme at One Light or One Dark from the theme preference.
/// System follows the OS appearance; Light/Dark force a fixed theme.
pub fn apply_theme(cx: &mut App) {
    let name = theme_name(cx);
    match theme::ThemeRegistry::global(cx).get(name) {
        Ok(theme) => theme::GlobalTheme::update_theme(cx, theme),
        Err(error) => log::error!("theme {name} unavailable: {error}"),
    }
}

fn theme_name(cx: &App) -> &'static str {
    let appearance = match xero_settings::snapshot(cx).theme {
        ThemeMode::Light => theme::Appearance::Light,
        ThemeMode::Dark => theme::Appearance::Dark,
        ThemeMode::System => theme::SystemAppearance::global(cx).0,
    };
    match appearance {
        theme::Appearance::Light => "One Light",
        theme::Appearance::Dark => "One Dark",
    }
}

/// Re-apply the theme when the OS light/dark setting changes (only matters when
/// preference is System). Keep the returned subscription alive (e.g. `.detach()`).
pub fn observe_appearance(window: &mut Window, _cx: &mut App) -> Subscription {
    window.observe_window_appearance(|window, cx| {
        *theme::SystemAppearance::global_mut(cx) =
            theme::SystemAppearance(cx.window_appearance().into());
        apply_theme(cx);
        window.refresh();
    })
}

fn app_version() -> semver::Version {
    semver::Version::new(
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0),
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0),
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0),
    )
}
