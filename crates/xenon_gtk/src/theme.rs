//! The user's theme, read from the same bundled theme files the GPUI app uses
//! (`crates/xenon_terminal/assets`), turned into a palette and one CSS sheet.
//! The sheet restyles libadwaita's named colors, so stock widgets follow the
//! theme without any painting code of ours.

use serde_json::Value;
use xenon_store::{AppSettings, ThemeMode};

const FAMILIES: [&str; 11] = [
    include_str!("../../xenon_terminal/assets/one.json"),
    include_str!("../../xenon_terminal/assets/nord.json"),
    include_str!("../../xenon_terminal/assets/solarized.json"),
    include_str!("../../xenon_terminal/assets/high_contrast.json"),
    include_str!("../../xenon_terminal/assets/neon.json"),
    include_str!("../../xenon_terminal/assets/tokyo.json"),
    include_str!("../../xenon_terminal/assets/mithril.json"),
    include_str!("../../xenon_terminal/assets/imperial.json"),
    include_str!("../../xenon_terminal/assets/gruvbox.json"),
    include_str!("../../xenon_terminal/assets/true_black.json"),
    include_str!("../../xenon_terminal/assets/hotdog.json"),
];

const ANSI: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

pub(crate) struct Palette {
    style: Value,
}

impl Palette {
    /// A color from the theme as `#rrggbb[aa]`, or `fallback` when missing.
    pub(crate) fn color(&self, key: &str, fallback: &str) -> String {
        self.style
            .get(key)
            .and_then(Value::as_str)
            .map_or_else(|| fallback.to_string(), str::to_string)
    }

    /// Terminal foreground, background, and the 16 ANSI colors.
    pub(crate) fn terminal(&self) -> (String, String, Vec<String>) {
        let fg = self.color("terminal.foreground", "#d8dee9");
        let bg = self.color("terminal.background", "#000000");
        let mut ansi: Vec<String> = ANSI
            .iter()
            .map(|n| self.color(&format!("terminal.ansi.{n}"), "#888888"))
            .collect();
        ansi.extend(
            ANSI.iter()
                .map(|n| self.color(&format!("terminal.ansi.bright_{n}"), "#cccccc")),
        );
        (fg, bg, ansi)
    }

    /// The sheet that themes the whole app.
    pub(crate) fn css(&self) -> String {
        let c = |key: &str| self.color(key, "#808080");
        let (bg, panel, fg, muted) = (
            c("background"),
            c("panel.background"),
            c("text"),
            c("text.muted"),
        );
        let (accent, border, selected) = (c("text.accent"), c("border"), c("element.selected"));
        let (warning, info, hover) = (c("warning"), c("info"), c("element.hover"));
        let elevated = c("elevated_surface.background");
        format!(
            "@define-color window_bg_color {bg}; @define-color window_fg_color {fg};\n\
             @define-color view_bg_color {bg}; @define-color view_fg_color {fg};\n\
             @define-color headerbar_bg_color {panel}; @define-color headerbar_fg_color {fg};\n\
             @define-color headerbar_border_color {border};\n\
             @define-color sidebar_bg_color {bg}; @define-color sidebar_fg_color {fg};\n\
             @define-color popover_bg_color {elevated}; @define-color popover_fg_color {fg};\n\
             @define-color dialog_bg_color {elevated}; @define-color dialog_fg_color {fg};\n\
             @define-color card_bg_color {panel}; @define-color card_fg_color {fg};\n\
             @define-color accent_bg_color {accent}; @define-color accent_color {accent};\n\
             @define-color accent_fg_color {bg};\n\
             headerbar {{ min-height: 36px; }}\n\
             .xsidebar, .xsidebar row, .xsidebar row:selected, .xsidebar row:hover {{ background: transparent; }}\n\
             .xsidebar {{ background: {bg}; }}\n\
             .xheading {{ color: {muted}; font-size: 0.78em; font-weight: 700; letter-spacing: 0.08em; }}\n\
             .xrow {{ border-radius: 6px; margin: 1px 6px; padding: 5px 8px 5px 6px; }}\n\
             .xrow:hover {{ background: {hover}; }}\n\
             .xrow.selected {{ background: {selected}; box-shadow: inset 2px 0 {fg}; }}\n\
             .xrow .xname {{ font-weight: 600; }}\n\
             .xrow .xpath {{ color: {muted}; font-size: 0.82em; }}\n\
             .xdot {{ min-width: 3px; border-radius: 2px; margin: 4px 0; }}\n\
             .xdot.working, .xrail.working {{ background: {info}; }}\n\
             .xdot.attention, .xrail.attention {{ background: {warning}; }}\n\
             .xtabs {{ background: {panel}; border-bottom: 1px solid {border}; }}\n\
             .xchip {{ padding: 0 2px 0 10px; min-width: 90px; }}\n\
             .xchip label {{ color: {muted}; }}\n\
             .xchip.selected label {{ color: {fg}; font-weight: 600; background: none; }}\n\
             .xchip.selected {{ background: {bg}; }}\n\
             .xchip:hover {{ background: {hover}; }}\n\
             .xchip .xclose {{ min-width: 18px; min-height: 18px; padding: 0; margin: 0; opacity: 0; }}\n\
             .xchip:hover .xclose, .xchip.selected .xclose {{ opacity: 0.8; }}\n\
             .xchip.exited label {{ font-style: italic; opacity: 0.6; }}\n\
             .xrail {{ min-height: 2px; margin-top: 3px; }}\n\
             .xchip.selected .xrail {{ background: {accent}; }}\n\
             .xpane {{ border-top: 2px solid transparent; }}\n\
             .xpane.ring {{ border-top-color: {accent}; }}\n\
             .xfind {{ background: {panel}; border-bottom: 1px solid {border}; padding: 4px 8px; }}\n\
             .xempty-title {{ font-size: 1.4em; font-weight: 600; }}\n\
             .xempty-hint {{ color: {muted}; }}\n\
             .xplaceholder {{ background: {bg}; color: {muted}; }}\n\
             paned > separator {{ min-width: 1px; min-height: 1px; background: {border}; }}\n\
             paned > separator:hover {{ background: {accent}; }}\n"
        )
    }
}

/// The palette the settings ask for. `system_dark` says what the OS wants when
/// the mode is System.
pub(crate) fn palette(settings: &AppSettings, system_dark: bool) -> Palette {
    let dark = match settings.theme {
        ThemeMode::System => system_dark,
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
    };
    let name = if dark {
        &settings.dark_theme
    } else {
        &settings.light_theme
    };
    find(name)
        .or_else(|| find(if dark { "One Dark" } else { "One Light" }))
        .unwrap_or(Palette { style: Value::Null })
}

fn find(name: &str) -> Option<Palette> {
    for text in FAMILIES {
        let family: Value = serde_json::from_str(text).ok()?;
        for theme in family.get("themes")?.as_array()? {
            if theme.get("name").and_then(Value::as_str) == Some(name) {
                let style = theme.get("style")?.clone();
                return Some(Palette { style });
            }
        }
    }
    None
}
