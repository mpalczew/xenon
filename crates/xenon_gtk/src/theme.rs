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
             @define-color headerbar_backdrop_color {panel};\n\
             @define-color sidebar_bg_color {panel}; @define-color sidebar_fg_color {fg};\n\
             @define-color popover_bg_color {elevated}; @define-color popover_fg_color {fg};\n\
             @define-color dialog_bg_color {elevated}; @define-color dialog_fg_color {fg};\n\
             @define-color card_bg_color {panel}; @define-color card_fg_color {fg};\n\
             @define-color accent_bg_color {accent}; @define-color accent_color {accent};\n\
             @define-color accent_fg_color {bg};\n\
             headerbar {{ min-height: 40px; box-shadow: none; border-bottom: 1px solid {border}; }}\n\
             headerbar button.flat {{ min-width: 30px; min-height: 30px; padding: 0; border-radius: 6px; color: {muted}; }}\n\
             headerbar button.flat:hover {{ color: {fg}; background: {hover}; }}\n\
             headerbar windowtitle .title {{ font-weight: 600; font-size: 0.95em; }}\n\
             headerbar windowtitle .subtitle {{ font-size: 0.8em; color: {muted}; opacity: 1; }}\n\
             vte-terminal {{ padding: 4px 8px; }}\n\
             .xsidebar {{ background: {panel}; }}\n\
             .xsidebar row, .xsidebar row:selected, .xsidebar row:hover, .xsidebar row:focus {{ background: transparent; outline: none; }}\n\
             .xsection-head {{ margin: 12px 14px 4px 14px; }}\n\
             .xheading {{ color: {muted}; font-size: 0.74em; font-weight: 700; letter-spacing: 0.1em; }}\n\
             .xsection button.flat {{ min-width: 24px; min-height: 24px; padding: 0; color: {muted}; border-radius: 6px; }}\n\
             .xsection button.flat:hover {{ color: {fg}; background: {hover}; }}\n\
             .xrow {{ border-radius: 6px; margin: 1px 8px; padding: 6px 8px 6px 6px; }}\n\
             .xrow:hover {{ background: {hover}; }}\n\
             .xrow.selected {{ background: {selected}; box-shadow: inset 2px 0 {fg}; border-radius: 0 6px 6px 0; }}\n\
             .xrow .xname {{ font-weight: 600; font-size: 0.95em; }}\n\
             .xrow .xpath {{ color: {muted}; font-size: 0.78em; }}\n\
             .xrow-icon {{ color: {muted}; margin: 2px 8px 0 2px; -gtk-icon-size: 15px; }}\n\
             .xrow.selected .xrow-icon {{ color: {accent}; }}\n\
             .xdot {{ min-width: 3px; border-radius: 2px; margin: 6px 0; }}\n\
             .xdot.working, .xrail.working {{ background: {info}; }}\n\
             .xdot.attention, .xrail.attention {{ background: {warning}; }}\n\
             .xfiles {{ padding: 2px 0 8px 0; }}\n\
             .xfiles row {{ border-radius: 6px; margin: 0 8px; min-height: 26px; }}\n\
             .xfiles row:hover {{ background: {hover}; }}\n\
             .xfiles row:selected {{ background: {selected}; box-shadow: inset 2px 0 {fg}; border-radius: 0 6px 6px 0; }}\n\
             .xfiles row.xopen label {{ color: {accent}; font-weight: 600; }}\n\
             .xfile {{ padding: 2px 6px 2px 0; }}\n\
             .xfile label {{ font-size: 0.92em; }}\n\
             .xfile image {{ color: {muted}; -gtk-icon-size: 14px; }}\n\
             .xfile .xchevron {{ -gtk-icon-size: 10px; opacity: 0.8; }}\n\
             .xfile .xlink {{ font-style: italic; }}\n\
             .xtabs {{ background: {panel}; border-bottom: 1px solid {border}; min-height: 32px; }}\n\
             .xchip {{ padding: 0 4px 0 12px; min-width: 96px; }}\n\
             .xchip label {{ color: {muted}; font-size: 0.92em; }}\n\
             .xchip.selected label {{ color: {fg}; font-weight: 600; background: none; }}\n\
             .xchip.selected {{ background: {bg}; }}\n\
             .xchip:hover {{ background: {hover}; }}\n\
             .xchip .xclose {{ min-width: 20px; min-height: 20px; padding: 0; margin: 0; opacity: 0; border-radius: 5px; }}\n\
             .xchip:hover .xclose, .xchip.selected .xclose {{ opacity: 0.7; }}\n\
             .xchip.exited label {{ font-style: italic; opacity: 0.6; }}\n\
             .xrail {{ min-height: 2px; margin-top: 2px; }}\n\
             .xchip.selected .xrail {{ background: {accent}; }}\n\
             .xtabs > button.flat {{ min-width: 28px; min-height: 28px; padding: 0; margin: 2px 4px; color: {muted}; border-radius: 6px; }}\n\
             .xtabs > button.flat:hover {{ color: {fg}; background: {hover}; }}\n\
             .xpane {{ border-top: 2px solid transparent; }}\n\
             .xpane.ring {{ border-top-color: {accent}; }}\n\
             .xfind {{ background: {panel}; border-bottom: 1px solid {border}; padding: 4px 8px; }}\n\
             .xempty-title {{ font-size: 1.4em; font-weight: 600; }}\n\
             .xempty-hint {{ color: {muted}; }}\n\
             .xplaceholder {{ background: {bg}; color: {muted}; }}\n\
             .xscrim {{ background: alpha(black, 0.38); }}\n\
             .xpalette {{ background: {elevated}; border: 1px solid {border}; border-radius: 10px; box-shadow: 0 16px 48px alpha(black, 0.55); }}\n\
             .xpalette entry.xpalette-entry {{ background: transparent; border: none; box-shadow: none; outline: none; min-height: 44px; padding: 0 16px; font-size: 1.1em; border-radius: 10px 10px 0 0; border-bottom: 1px solid {border}; }}\n\
             .xpalette listview.xpalette-list {{ background: transparent; padding: 6px; }}\n\
             .xpalette listview.xpalette-list > row {{ border-radius: 6px; padding: 0; min-height: 34px; background: transparent; }}\n\
             .xpalette listview.xpalette-list > row:selected {{ background: {selected}; box-shadow: inset 2px 0 {accent}; border-radius: 0 6px 6px 0; }}\n\
             .xp-row {{ padding: 6px 12px; }}\n\
             .xp-title {{ font-size: 0.97em; }}\n\
             .xp-detail {{ color: {muted}; font-size: 0.82em; }}\n\
             .xp-badge {{ color: {muted}; font-size: 0.72em; font-weight: 700; letter-spacing: 0.06em; padding: 1px 6px; border: 1px solid {border}; border-radius: 4px; }}\n\
             .xp-empty {{ color: {muted}; padding: 14px 16px; }}\n\
             .xp-hint {{ color: {muted}; font-size: 0.78em; padding: 7px 16px 9px 16px; border-top: 1px solid {border}; }}\n\
             .xeditor-scroll {{ background: {bg}; }}\n\
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
