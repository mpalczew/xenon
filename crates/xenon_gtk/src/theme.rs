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
    /// A light theme (status colors are pulled darker to stay legible).
    light: bool,
}

impl Palette {
    /// A color from the theme as `#rrggbb[aa]`, or `fallback` when missing.
    pub(crate) fn color(&self, key: &str, fallback: &str) -> String {
        self.style
            .get(key)
            .and_then(Value::as_str)
            .map_or_else(|| fallback.to_string(), str::to_string)
    }

    /// A syntax color (`keyword`, `string`) as `#rrggbb[aa]`.
    pub(crate) fn syntax(&self, name: &str) -> Option<String> {
        self.style
            .get("syntax")?
            .get(name)?
            .get("color")?
            .as_str()
            .map(str::to_string)
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

    /// The sheet that themes the whole app (one long format string).
    #[allow(clippy::too_many_lines)]
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
        let (added, deleted, renamed, conflict) = (
            c("version_control.added"),
            c("version_control.deleted"),
            self.color("version_control.renamed", "#74ade8"),
            self.color("version_control.conflict", "#dec184"),
        );
        let modified = c("version_control.modified");
        // Status yellow vanishes on a light canvas: pull it down there.
        let modified = if self.light {
            format!("shade({modified}, 0.62)")
        } else {
            modified
        };
        let (editor_bg, editor_fg) = (
            self.color("editor.background", &bg),
            self.color("editor.foreground", &fg),
        );
        let element_bg = c("element.background");
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
             headerbar {{ min-height: 40px; box-shadow: none; border-bottom: 1px solid {border}; background-color: {panel}; color: {fg}; }}\n\
             headerbar:backdrop {{ background-color: {panel}; color: {muted}; }}\n\
             headerbar:backdrop windowtitle {{ opacity: 0.8; }}\n\
             headerbar button.flat {{ min-width: 30px; min-height: 30px; padding: 0; border-radius: 6px; color: {muted}; }}\n\
             headerbar button.flat:hover {{ color: {fg}; background: {hover}; }}\n\
             headerbar windowtitle .title {{ font-weight: 600; font-size: 0.95em; }}\n\
             headerbar windowtitle .subtitle {{ font-size: 0.8em; color: {muted}; opacity: 1; }}\n\
             vte-terminal {{ padding: 4px 8px; }}\n\
             .xsidebar {{ background: {panel}; }}\n\
             .xsidebar row, .xsidebar row:selected, .xsidebar row:hover, .xsidebar row:focus {{ background: transparent; outline: none; }}\n\
             .xsection-head {{ min-height: 52px; padding: 0 16px; }}\n\
             .xsection-head.xfirst {{ min-height: 60px; padding-right: 8px; }}\n\
             .xheading {{ color: {muted}; font-size: 0.74em; font-weight: 700; letter-spacing: 0.1em; }}\n\
             .xsection button.flat {{ min-width: 24px; min-height: 24px; padding: 0; color: {muted}; border-radius: 6px; }}\n\
             .xsection button.flat:hover {{ color: {fg}; background: {hover}; }}\n\
             .xsidebar row.drop-before {{ box-shadow: 0 -1px 0 1px {accent}; }}\n\
             .xsidebar row.drop-after {{ box-shadow: 0 1px 0 1px {accent}; }}\n\
             .xrow {{ border-radius: 6px; margin: 1px 8px; padding: 7px 8px; }}\n\
             .xrow:hover {{ background: {hover}; }}\n\
             .xrow.selected {{ background: {selected}; box-shadow: inset 2px 0 {fg}; border-radius: 0 6px 6px 0; }}\n\
             .xrow .xname {{ font-weight: 600; font-size: 0.95em; }}\n\
             .xrow .xpath {{ color: {muted}; font-size: 0.78em; }}\n\
             .xrow-icon {{ color: {muted}; margin: 2px 8px 0 2px; -gtk-icon-size: 15px; }}\n\
             .xrow.selected .xrow-icon {{ color: {accent}; }}\n\
             .xdot {{ min-width: 3px; border-radius: 2px; margin: 6px 0; }}\n\
             .xdot.working {{ background: {info}; }}\n\
             .xdot.attention {{ background: {warning}; }}\n\
             .xfiles {{ padding: 2px 0 8px 0; }}\n\
             .xfiles row {{ border-radius: 6px; margin: 0 8px; min-height: 26px; }}\n\
             .xfiles row:hover {{ background: {hover}; }}\n\
             .xfiles row:selected {{ background: {selected}; box-shadow: inset 2px 0 {fg}; border-radius: 0 6px 6px 0; }}\n\
             .xfiles row.xopen label {{ color: {accent}; font-weight: 600; }}\n\
             .xfile {{ padding: 2px 8px 2px 0; }}\n\
             .xfile label {{ font-size: 0.92em; }}\n\
             .xfile image {{ color: {muted}; -gtk-icon-size: 14px; }}\n\
             .xfile .xchevron {{ -gtk-icon-size: 10px; opacity: 0.8; min-width: 14px; }}\n\
             .xfile .xficon {{ min-width: 16px; }}\n\
             .xfile .xlink {{ font-style: italic; }}\n\
             .xdrop {{ background: alpha({accent}, 0.35); border: 2px solid {accent}; }}\n\
             .xpane {{ border-top: 2px solid transparent; }}\n\
             .xpane.ring {{ border-top-color: {accent}; }}\n\
             .xfind {{ background: {panel}; border-bottom: 1px solid {border}; padding: 4px 8px; }}\n\
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
             .xgutter {{ min-width: 20px; margin-right: 0; }}\n\
             .xrow-close {{ min-width: 20px; min-height: 20px; padding: 0; opacity: 0; }}\n\
             .xrow:hover .xrow-close, .xrow-close:focus {{ opacity: 0.8; }}\n\
             .xrow:hover .xtick {{ opacity: 0; }}\n\
             .xhints {{ border-top: 1px solid {border}; padding-top: 12px; margin-top: 4px; }}\n\
             .xhints-head {{ color: {muted}; font-size: 0.82em; margin-bottom: 4px; }}\n\
             .xhints-name {{ color: {muted}; font-size: 0.85em; }}\n\
             .xhints-keys {{ font-size: 0.85em; font-weight: 600; }}\n\
             .xtick {{ min-width: 5px; min-height: 18px; border-radius: 3px; }}\n\
             .xtick.dirty {{ background: {modified}; }}\n\
             .xtick.index {{ border: 2px solid {modified}; }}\n\
             .xsection button.xgit-head {{ margin: 0; padding: 0 16px; min-height: 52px; border-radius: 0; }}\n\
             .xsection button.xfold {{ padding: 0; min-height: 52px; border-radius: 0; }}\n\
             .xsection-head.xfirst button.xfold {{ min-height: 60px; }}\n\
             .xsection button.xgit-head {{ padding: 0 16px; }}\n\
             .xsection button.xfold:hover {{ background: transparent; }}\n\
             .xfold:hover .xheading {{ color: {fg}; }}\n\
             .xgit-section {{ border-top: 1px solid {border}; }}\n\
             .xgit-branch {{ color: {muted}; font-size: 0.85em; margin-right: 6px; }}\n\
             .xcount {{ background: {selected}; margin: 0 0 0 0; color: {fg}; font-size: 0.78em; font-weight: 600; border-radius: 10px; padding: 0 7px; min-width: 6px; }}\n\
             .xgit {{ padding: 2px 0 6px 0; }}\n\
             .xgit row {{ border-radius: 6px; margin: 0 8px; min-height: 28px; }}\n\
             .xgit row:hover {{ background: {hover}; }}\n\
             .xgit row:selected, .xgit row.xopen {{ background: {selected}; box-shadow: inset 2px 0 {fg}; border-radius: 0 6px 6px 0; }}\n\
             .xgit-row {{ padding: 0 8px; }}\n\
             .xgit-name {{ font-size: 0.95em; }}\n\
             .xgit-dir {{ color: {muted}; font-size: 0.82em; }}\n\
             .xgit-clean {{ padding: 4px 14px 8px 14px; }}\n\
             .xdeleted {{ text-decoration: line-through; }}\n\
             .xbadge {{ min-width: 18px; min-height: 18px; border-radius: 9px; font-size: 0.68em; font-weight: 700; padding: 0; }}\n\
             .xbadge.xk-modified {{ background: alpha({modified}, 0.18); }}\n\
             .xbadge.xk-added {{ background: alpha({added}, 0.18); }}\n\
             .xbadge.xk-deleted {{ background: alpha({deleted}, 0.18); }}\n\
             .xbadge.xk-renamed {{ background: alpha({renamed}, 0.18); }}\n\
             .xbadge.xk-conflict {{ background: alpha({conflict}, 0.18); }}\n\
             .xbadge.xk-untracked {{ background: alpha({muted}, 0.18); color: {muted}; }}\n\
             .xk-modified {{ color: {modified}; }}\n\
             .xk-added {{ color: {added}; }}\n\
             .xk-deleted {{ color: {deleted}; }}\n\
             .xk-renamed {{ color: {renamed}; }}\n\
             .xk-conflict {{ color: {conflict}; }}\n\
             .xcell {{ min-width: 4px; min-height: 10px; border-radius: 1px; }}\n\
             .xcell.add {{ background: {added}; }}\n\
             .xcell.del {{ background: {deleted}; }}\n\
             .xcell.neutral {{ background: alpha({muted}, 0.3); }}\n\
             .xdiff {{ background: {editor_bg}; }}\n\
             .xdiff-head {{ background: {panel}; border-bottom: 1px solid {border}; padding: 6px 16px; min-height: 28px; }}\n\
             .xdiff-path {{ font-size: 0.95em; }}\n\
             .xdiff-muted {{ color: {muted}; font-size: 0.85em; }}\n\
             .xdiff-head .xmode {{ padding: 2px 10px; min-height: 22px; font-size: 0.85em; }}\n\
             .xdiff-head .xmode:checked {{ background: {selected}; color: {fg}; font-weight: 600; }}\n\
             .xpill {{ border: 1px solid {border}; border-radius: 6px; padding: 2px 8px; color: {muted}; font-size: 0.85em; }}\n\
             .xdiff-banner {{ background: {element_bg}; border: 1px solid {border}; border-radius: 6px; margin: 12px 16px 0 16px; padding: 8px 12px; }}\n\
             .xdiff-note {{ color: {muted}; padding: 14px 16px; }}\n\
             textview.xdiff-code, textview.xdiff-code text {{ background: {editor_bg}; color: {editor_fg}; }}\n\
             .xdiff-half.right {{ border-left: 1px solid {border}; }}\n\
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
        .map(|p| Palette { light: !dark, ..p })
        .unwrap_or(Palette {
            style: Value::Null,
            light: !dark,
        })
}

pub(crate) fn find(name: &str) -> Option<Palette> {
    for text in FAMILIES {
        let family: Value = serde_json::from_str(text).ok()?;
        for theme in family.get("themes")?.as_array()? {
            if theme.get("name").and_then(Value::as_str) == Some(name) {
                let style = theme.get("style")?.clone();
                return Some(Palette {
                    style,
                    light: false,
                });
            }
        }
    }
    None
}
