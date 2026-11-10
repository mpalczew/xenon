use serde::{Deserialize, Serialize};

use crate::TerminalAutoClose;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowState {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

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
    pub(crate) const MIN_WIDTH: f32 = 400.0;
    pub(crate) const MIN_HEIGHT: f32 = 300.0;
    pub(crate) const MAX_EXTENT: f32 = 20_000.0;

    pub fn new(x: f32, y: f32, width: f32, height: f32, state: WindowState) -> Self {
        Self {
            x,
            y,
            width,
            height,
            state,
        }
    }

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub editor_font_size: f32,
    pub terminal_font_size: f32,
    pub ui_font_size: f32,
    pub editor_font_family: String,
    pub terminal_font_family: String,
    pub ui_font_family: String,
    pub show_line_numbers: bool,
    /// Soft wrap for markdown and other prose. The file is not rewritten.
    pub wrap_prose: bool,
    /// Soft wrap for every other text file.
    pub wrap_code: bool,
    pub vim_mode: bool,
    pub theme: ThemeMode,
    pub light_theme: String,
    pub dark_theme: String,
    pub terminal_auto_close: TerminalAutoClose,
    pub workspaces_collapsed: bool,
    pub files_open: bool,
    pub git_collapsed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_section_height: Option<f32>,
    pub sidebar_width: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspaces_section_height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowGeometry>,
    pub remote_enabled: bool,
    /// Hold a no-idle-sleep assertion while a phone is connected. Opt-in; the
    /// older `remote_keep_awake` key is ignored.
    pub remote_keep_awake_while_connected: bool,
    pub remote_port: u16,
    pub remote_hostname: String,
    pub prose: crate::ProseSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            editor_font_size: 14.0,
            terminal_font_size: 14.0,
            ui_font_size: 14.0,
            editor_font_family: DEFAULT_FONT_FAMILY.into(),
            terminal_font_family: DEFAULT_FONT_FAMILY.into(),
            ui_font_family: DEFAULT_UI_FONT_FAMILY.into(),
            show_line_numbers: true,
            wrap_prose: true,
            wrap_code: false,
            vim_mode: false,
            theme: ThemeMode::System,
            light_theme: DEFAULT_LIGHT_THEME.into(),
            dark_theme: DEFAULT_DARK_THEME.into(),
            terminal_auto_close: TerminalAutoClose::default(),
            workspaces_collapsed: false,
            files_open: true,
            git_collapsed: false,
            files_section_height: None,
            sidebar_width: 240.0,
            workspaces_section_height: None,
            window: None,
            remote_enabled: false,
            remote_keep_awake_while_connected: false,
            remote_port: DEFAULT_REMOTE_PORT,
            remote_hostname: String::new(),
            prose: crate::ProseSettings::default(),
        }
    }
}

pub const DEFAULT_FONT_FAMILY: &str = "Lilex";
pub const DEFAULT_UI_FONT_FAMILY: &str = ".SystemUIFont";
pub const DEFAULT_LIGHT_THEME: &str = "One Light";
pub const DEFAULT_DARK_THEME: &str = "One Dark";

pub const DEFAULT_REMOTE_PORT: u16 = 17890;
