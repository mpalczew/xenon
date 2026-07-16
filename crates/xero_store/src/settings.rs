//! Durable app settings (`settings.json` schema).

use serde::{Deserialize, Serialize};

use crate::TerminalAutoClose;

/// When to use the light vs dark theme: fixed, or track the OS appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Durable UI settings under `settings.json`. All fields default for forward-compat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_font_size")]
    pub editor_font_size: f32,
    #[serde(default = "default_font_size")]
    pub terminal_font_size: f32,
    #[serde(default = "default_font_size")]
    pub ui_font_size: f32,
    #[serde(default = "default_font_family")]
    pub editor_font_family: String,
    #[serde(default = "default_font_family")]
    pub terminal_font_family: String,
    #[serde(default = "default_ui_font_family")]
    pub ui_font_family: String,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub vim_mode: bool,
    /// System / Light / Dark: which appearance to apply (like Zed "Mode").
    #[serde(default)]
    pub theme: ThemeMode,
    /// Theme name used for light appearance (e.g. "One Light").
    #[serde(default = "default_light_theme")]
    pub light_theme: String,
    /// Theme name used for dark appearance (e.g. "One Dark").
    #[serde(default = "default_dark_theme")]
    pub dark_theme: String,
    /// Close terminal tabs when the shell process exits.
    #[serde(default)]
    pub terminal_auto_close: TerminalAutoClose,
    /// Left panel Workspaces section collapsed.
    #[serde(default)]
    pub workspaces_collapsed: bool,
    /// Left panel Files section expanded.
    #[serde(default = "default_true")]
    pub files_open: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            editor_font_size: default_font_size(),
            terminal_font_size: default_font_size(),
            ui_font_size: default_font_size(),
            editor_font_family: default_font_family(),
            terminal_font_family: default_font_family(),
            ui_font_family: default_ui_font_family(),
            show_line_numbers: true,
            vim_mode: false,
            theme: ThemeMode::System,
            light_theme: default_light_theme(),
            dark_theme: default_dark_theme(),
            terminal_auto_close: TerminalAutoClose::default(),
            workspaces_collapsed: false,
            files_open: true,
        }
    }
}

fn default_font_size() -> f32 {
    14.0
}

fn default_font_family() -> String {
    DEFAULT_FONT_FAMILY.into()
}

fn default_ui_font_family() -> String {
    DEFAULT_UI_FONT_FAMILY.into()
}

fn default_true() -> bool {
    true
}

/// Default monospaced face on macOS.
pub const DEFAULT_FONT_FAMILY: &str = "Menlo";
/// Default UI (chrome) face: platform system UI font.
pub const DEFAULT_UI_FONT_FAMILY: &str = ".SystemUIFont";
/// Default light theme name (One Light).
pub const DEFAULT_LIGHT_THEME: &str = "One Light";
/// Default dark theme name (One Dark, black surfaces in our vendor).
pub const DEFAULT_DARK_THEME: &str = "One Dark";

fn default_light_theme() -> String {
    DEFAULT_LIGHT_THEME.into()
}

fn default_dark_theme() -> String {
    DEFAULT_DARK_THEME.into()
}
