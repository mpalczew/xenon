mod agents;
mod choices;
mod edits;
mod families;
mod setters;
mod values;

pub use xenon_store::{AppSettings, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME};

pub use agents::{skill_homes, skill_refresh_label, skill_subtitle};
pub use choices::{
    FontChoice, PROSE_FONTS, PROSE_LINE_HEIGHTS, PROSE_SIZES, format_size, mono_font_choices,
    nearest_index,
};
pub use families::FontProbe;
pub use setters::Surface;

pub use xenon_store::{
    MarkdownMode, ProseFilePrefs, ProseFont, ProseMarkers, ProseSettings, TerminalAutoClose,
    ThemeMode, prose_file_prefs, set_prose_file_prefs,
};

pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 32.0;
pub(crate) const DEFAULT_FONT_SIZE: f32 = 14.0;
pub(crate) const DEFAULT_SHOW_LINE_NUMBERS: bool = true;
pub(crate) const DEFAULT_VIM_MODE: bool = false;
pub use values::UiSettings;

pub(crate) use families::{ensure_mono_family, ensure_ui_family};
