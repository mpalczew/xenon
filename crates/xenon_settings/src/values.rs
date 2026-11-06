//! The live settings values and how they load from and save to `settings.json`.

use xenon_store::{DEFAULT_FONT_FAMILY, DEFAULT_UI_FONT_FAMILY};

use crate::{
    AppSettings, DEFAULT_DARK_THEME, DEFAULT_FONT_SIZE, DEFAULT_LIGHT_THEME,
    DEFAULT_SHOW_LINE_NUMBERS, DEFAULT_VIM_MODE, FontProbe, MAX_FONT_SIZE, MIN_FONT_SIZE,
    ProseSettings, TerminalAutoClose, ThemeMode, ensure_mono_family, ensure_ui_family,
};

/// Font size + family for one surface (editor, terminal, or UI chrome).
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFont {
    pub size: f32,
    pub family: String,
}

impl Default for FaceFont {
    fn default() -> Self {
        Self {
            size: DEFAULT_FONT_SIZE,
            family: DEFAULT_FONT_FAMILY.into(),
        }
    }
}

impl FaceFont {
    pub(crate) fn default_ui() -> Self {
        Self {
            size: DEFAULT_FONT_SIZE,
            family: DEFAULT_UI_FONT_FAMILY.into(),
        }
    }

    pub(crate) fn nudge(&mut self, delta: f32) {
        self.size = clamp_size(self.size + delta);
    }
}

pub(crate) fn clamp_size(size: f32) -> f32 {
    size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

/// Theme mode and the chosen light/dark theme names.
#[derive(Clone, Debug)]
pub struct ThemePreference {
    pub mode: ThemeMode,
    pub light: String,
    pub dark: String,
}

impl Default for ThemePreference {
    fn default() -> Self {
        Self {
            mode: ThemeMode::System,
            light: DEFAULT_LIGHT_THEME.into(),
            dark: DEFAULT_DARK_THEME.into(),
        }
    }
}

/// Live values for every app-wide setting.
#[derive(Clone, Debug)]
pub struct UiSettings {
    pub editor: FaceFont,
    pub terminal: FaceFont,
    pub ui: FaceFont,
    pub show_line_numbers: bool,
    pub wrap_prose: bool,
    pub wrap_code: bool,
    pub prose: ProseSettings,
    pub vim_mode: bool,
    pub terminal_auto_close: TerminalAutoClose,
    pub theme: ThemePreference,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            editor: FaceFont::default(),
            terminal: FaceFont::default(),
            ui: FaceFont::default_ui(),
            show_line_numbers: DEFAULT_SHOW_LINE_NUMBERS,
            wrap_prose: true,
            wrap_code: false,
            prose: ProseSettings::default(),
            vim_mode: DEFAULT_VIM_MODE,
            terminal_auto_close: TerminalAutoClose::default(),
            theme: ThemePreference::default(),
        }
    }
}

impl UiSettings {
    /// Build live values from a loaded snapshot. Editor/terminal families must
    /// resolve as monospaced; UI may be any face. The flag is true when a stored
    /// family was healed, so the caller should persist.
    pub fn resolve(settings: &AppSettings, probe: &dyn FontProbe) -> (Self, bool) {
        let editor = ensure_mono_family(&settings.editor_font_family, probe);
        let terminal = ensure_mono_family(&settings.terminal_font_family, probe);
        let ui = ensure_ui_family(&settings.ui_font_family, probe);
        let healed = editor != settings.editor_font_family
            || terminal != settings.terminal_font_family
            || ui != settings.ui_font_family;
        let face = |size: f32, family: String| FaceFont {
            size: clamp_size(size),
            family,
        };
        let live = Self {
            editor: face(settings.editor_font_size, editor),
            terminal: face(settings.terminal_font_size, terminal),
            ui: face(settings.ui_font_size, ui),
            show_line_numbers: settings.show_line_numbers,
            wrap_prose: settings.wrap_prose,
            wrap_code: settings.wrap_code,
            prose: sanitize_prose(settings.prose.clone()),
            vim_mode: settings.vim_mode,
            terminal_auto_close: settings.terminal_auto_close,
            theme: ThemePreference {
                mode: settings.theme,
                light: settings.light_theme.clone(),
                dark: settings.dark_theme.clone(),
            },
        };
        (live, healed)
    }

    /// Write these values over `settings`, leaving fields we do not own (such
    /// as chrome section prefs) untouched.
    pub fn write_into(&self, settings: &mut AppSettings) {
        settings.editor_font_size = self.editor.size;
        settings.editor_font_family.clone_from(&self.editor.family);
        settings.terminal_font_size = self.terminal.size;
        settings
            .terminal_font_family
            .clone_from(&self.terminal.family);
        settings.ui_font_size = self.ui.size;
        settings.ui_font_family.clone_from(&self.ui.family);
        settings.show_line_numbers = self.show_line_numbers;
        settings.wrap_prose = self.wrap_prose;
        settings.wrap_code = self.wrap_code;
        settings.prose = self.prose.clone();
        settings.vim_mode = self.vim_mode;
        settings.theme = self.theme.mode;
        settings.light_theme.clone_from(&self.theme.light);
        settings.dark_theme.clone_from(&self.theme.dark);
        settings.terminal_auto_close = self.terminal_auto_close;
    }

    /// Current values merged over the settings on disk, so a save does not
    /// reset chrome-only prefs (Workspaces/Files collapse).
    pub fn snapshot(&self) -> AppSettings {
        let mut settings = xenon_store::load_settings().unwrap_or_default();
        self.write_into(&mut settings);
        settings
    }

    /// Persist to disk. Logs on failure; never panics.
    pub fn persist(&self) {
        if let Err(error) = xenon_store::update_settings(|settings| self.write_into(settings)) {
            log::error!("save settings failed: {error}");
        }
    }
}

pub(crate) fn sanitize_prose(mut value: ProseSettings) -> ProseSettings {
    value.size = value.size.clamp(12.0, 36.0);
    value.line_height = value.line_height.clamp(1.2, 2.2);
    value.measure = value.measure.clamp(40.0, 120.0);
    value
}
