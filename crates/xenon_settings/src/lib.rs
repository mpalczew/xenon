//! App-wide UI settings as plain data, shared by every frontend.
//! Durable values live in `~/.xenon/settings.json` (via `xenon_store`).
//!
//! `UiSettings` holds the live values and the rules that change them (clamps,
//! nudges, wrap defaults). A frontend keeps one in whatever global it likes
//! and calls `persist` after a change.

mod edits;
mod families;
mod fonts;
mod values;

pub use xenon_store::{AppSettings, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME};

pub use families::{
    FontProbe, canonicalize_mono_family, display_mono_family, display_ui_family,
    ensure_mono_family, ensure_ui_family,
};
pub use fonts::EMBEDDED_FONTS;
pub use xenon_store::{
    MarkdownMode, ProseFilePrefs, ProseFont, ProseMarkers, ProseSettings, TerminalAutoClose,
    ThemeMode, prose_file_prefs, set_prose_file_prefs,
};

pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 32.0;
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_SHOW_LINE_NUMBERS: bool = true;
pub const DEFAULT_VIM_MODE: bool = false;
pub use values::{FaceFont, ThemePreference, UiSettings};
