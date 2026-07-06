//! One-time initialization of the zed globals that `terminal` depends on.
//!
//! `terminal::TerminalBuilder` reads `release_channel::AppVersion`,
//! `TerminalSettings`, and font settings from `ThemeSettings`, all of which are
//! gpui globals. They must be installed before the first terminal spawns.

use gpui::App;
use settings::Settings;

/// Install every global the terminal backend needs. Call once, at startup,
/// inside `application().run(|cx| ...)`.
pub fn init(cx: &mut App) {
    settings::init(cx);
    release_channel::init(app_version(), cx);
    theme::init(theme::LoadThemes::JustBase, cx);
    theme_settings::ThemeSettings::register(cx);
    terminal::terminal_settings::TerminalSettings::register(cx);
}

fn app_version() -> semver::Version {
    semver::Version::new(
        env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0),
        env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0),
        env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0),
    )
}
