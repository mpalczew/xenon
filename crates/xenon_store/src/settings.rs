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

/// Windowed / maximized / fullscreen (restore size is always the rect).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowState {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

/// Last main-window geometry (pixels, global coords).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub state: WindowState,
}

impl WindowGeometry {
    /// Minimum restored size so a tiny/corrupt rect never traps the UI.
    pub const MIN_WIDTH: f32 = 400.0;
    pub const MIN_HEIGHT: f32 = 300.0;
    /// Reject absurd values (corrupt file / multi-monitor coordinate blowup).
    pub const MAX_EXTENT: f32 = 20_000.0;

    pub fn new(x: f32, y: f32, width: f32, height: f32, state: WindowState) -> Self {
        Self {
            x,
            y,
            width,
            height,
            state,
        }
    }

    /// True when size and origin look usable after a restart.
    pub fn is_sane(&self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && self.x.is_finite()
            && self.y.is_finite()
            && self.width >= Self::MIN_WIDTH
            && self.height >= Self::MIN_HEIGHT
            && self.width <= Self::MAX_EXTENT
            && self.height <= Self::MAX_EXTENT
            && self.x.abs() <= Self::MAX_EXTENT
            && self.y.abs() <= Self::MAX_EXTENT
    }
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
    /// Last main window position/size (None until the user has moved/resized once).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowGeometry>,
    /// Mobile remote shared password (phone auth token). Empty until first enable or set.
    #[serde(default)]
    pub remote_password: String,
    /// Mobile remote bind port (stable across restarts).
    #[serde(default = "default_remote_port")]
    pub remote_port: u16,
    /// Optional host for copyable phone URLs (MagicDNS name, LAN hostname, etc.).
    #[serde(default)]
    pub remote_hostname: String,
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
            window: None,
            remote_password: String::new(),
            remote_port: default_remote_port(),
            remote_hostname: String::new(),
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

/// Default mobile remote listen port (not ephemeral; survives restart).
pub const DEFAULT_REMOTE_PORT: u16 = 17890;

fn default_remote_port() -> u16 {
    DEFAULT_REMOTE_PORT
}
