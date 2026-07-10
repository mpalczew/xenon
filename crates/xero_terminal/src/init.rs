//! One-time initialization of the zed globals that `terminal` depends on.
//!
//! `terminal::TerminalBuilder` reads `release_channel::AppVersion`,
//! `TerminalSettings`, and font settings from `ThemeSettings`, all of which are
//! gpui globals. They must be installed before the first terminal spawns.

use gpui::{App, SharedString, Subscription, Window};
use settings::Settings;
use xero_settings::ThemeMode;

/// Bundled theme families. Zed-derived ones: see ATTRIBUTION.md.
const THEME_FILES: &[&[u8]] = &[
    // Classic / zed-derived
    include_bytes!("../assets/one.json"),
    include_bytes!("../assets/ayu.json"),
    include_bytes!("../assets/gruvbox.json"),
    include_bytes!("../assets/solarized.json"),
    include_bytes!("../assets/nord.json"),
    // IDE familiarity
    include_bytes!("../assets/vscode.json"),
    include_bytes!("../assets/intellij.json"),
    include_bytes!("../assets/xcode.json"),
    // Defaults / a11y
    include_bytes!("../assets/high_contrast.json"),
    // Brand pack
    include_bytes!("../assets/neon.json"),
    include_bytes!("../assets/abyss.json"),
    include_bytes!("../assets/tokyo.json"),
    include_bytes!("../assets/runner.json"),
    include_bytes!("../assets/aurora.json"),
    include_bytes!("../assets/ember.json"),
    // Culture
    include_bytes!("../assets/ink.json"),
    // Personality / fun
    include_bytes!("../assets/imperial.json"),
    include_bytes!("../assets/mithril.json"),
    include_bytes!("../assets/synthwave.json"),
    include_bytes!("../assets/radioactive.json"),
    include_bytes!("../assets/hotdog.json"),
];

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

/// Load bundled theme families, then select per the current theme preference.
fn load_themes(cx: &mut App) {
    let mut families = Vec::new();
    for bytes in THEME_FILES {
        match theme_settings::deserialize_user_theme(bytes) {
            Ok(content) => families.push(theme_settings::refine_theme_family(content)),
            Err(error) => log::error!("failed to parse bundled theme: {error}"),
        }
    }
    if !families.is_empty() {
        theme::ThemeRegistry::global(cx).insert_theme_families(families);
    }
    apply_theme(cx);
}

/// Point the global theme at the light or dark theme name from preference.
/// System follows the OS appearance; Light/Dark force a fixed appearance.
pub fn apply_theme(cx: &mut App) {
    let name = active_theme_name(cx);
    match theme::ThemeRegistry::global(cx).get(&name) {
        Ok(theme) => {
            theme::GlobalTheme::update_theme(cx, theme);
            refresh_windows(cx);
        }
        Err(error) => log::error!("theme {name} unavailable: {error}"),
    }
}

/// Theme names registered for a given appearance, sorted.
pub fn theme_names(appearance: theme::Appearance, cx: &App) -> Vec<SharedString> {
    let mut names: Vec<_> = theme::ThemeRegistry::global(cx)
        .list()
        .into_iter()
        .filter(|meta| meta.appearance == appearance)
        .map(|meta| meta.name)
        .collect();
    names.sort();
    names
}

fn active_theme_name(cx: &App) -> String {
    let settings = xero_settings::snapshot(cx);
    let appearance = match settings.theme {
        ThemeMode::Light => theme::Appearance::Light,
        ThemeMode::Dark => theme::Appearance::Dark,
        ThemeMode::System => theme::SystemAppearance::global(cx).0,
    };
    match appearance {
        theme::Appearance::Light => settings.light_theme,
        theme::Appearance::Dark => settings.dark_theme,
    }
}

/// Refresh every open window (settings changes that affect the main UI).
pub fn refresh_windows(cx: &mut App) {
    for window in cx.windows() {
        let _ = window.update(cx, |_, window, _| window.refresh());
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
