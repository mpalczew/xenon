// Terminal palette in `terminal` is copied from Ptyxis: see ATTRIBUTION.md, "Ptyxis GNOME palette".
//! The look is the desktop's: stock libadwaita colors, backgrounds and accent,
//! following light and dark through `adw::StyleManager`. What is left is the
//! static sheet for our own widgets and two small light or dark switches, the
//! terminal palette here and the GtkSourceView scheme (`scheme`).

/// Rules for widgets of ours that have no stock equivalent. Colors are
/// libadwaita's named colors or the text color at low strength.
pub(crate) const CSS: &str = "\
listview.xfiles > row { min-height: 24px; padding: 0 6px; }
listview.xfiles treeexpander:drop(active) { background: alpha(@accent_color, 0.25); }
listview.xfiles:drop(active) { box-shadow: inset 0 0 0 2px @accent_color; }
vte-terminal { padding: 4px 8px; }
row.drop-before { box-shadow: 0 -1px 0 1px @accent_color; }
row.drop-after { box-shadow: 0 1px 0 1px @accent_color; }
.xdrop { background: alpha(@accent_color, 0.35); border: 2px solid @accent_color; }
.xpane { border-top: 2px solid transparent; }
.xpane:focus-within:not(:only-child) { border-top-color: @accent_color; }
.xfind { border-bottom: 1px solid @borders; padding: 4px 8px; }
.xdiff-head { border-bottom: 1px solid @borders; padding: 6px 16px; min-height: 28px; }
.xpill { border: 1px solid @borders; border-radius: 6px; padding: 2px 8px; }
.xdiff-banner { background: alpha(currentColor, 0.06); border: 1px solid @borders; border-radius: 6px; margin: 12px 16px 0 16px; padding: 8px 12px; }
.xdiff-note { padding: 14px 16px; }
.xdiff-half.right { border-left: 1px solid @borders; }
";

/// The GtkSourceView scheme that goes with the desktop's light or dark.
pub(crate) fn scheme(dark: bool) -> Option<sourceview::StyleScheme> {
    sourceview::StyleSchemeManager::default().scheme(if dark { "Adwaita-dark" } else { "Adwaita" })
}

/// Whether the desktop is showing its dark look.
pub(crate) fn dark() -> bool {
    adw::StyleManager::default().is_dark()
}

/// Terminal foreground, background and the 16 ANSI colors for light or dark.
/// Both come from the default "GNOME" palette of Ptyxis (GPL-3.0-or-later),
/// its Light and Dark variants, with that palette's own foreground and
/// background. See ATTRIBUTION.md, entry "Ptyxis GNOME palette".
pub(crate) fn terminal(dark: bool) -> (&'static str, &'static str, [&'static str; 16]) {
    const LIGHT: [&str; 16] = [
        "#1d1d20", "#c01c28", "#26a269", "#a2734c", "#12488b", "#a347ba", "#2aa1b3", "#cfcfcf",
        "#5d5d5d", "#f66151", "#33d17a", "#e9ad0c", "#2a7bde", "#c061cb", "#33c7de", "#ffffff",
    ];
    const DARK: [&str; 16] = [
        "#241f31", "#c01c28", "#2ec27e", "#f5c211", "#1e78e4", "#9841bb", "#0ab9dc", "#c0bfbc",
        "#5e5c64", "#ed333b", "#57e389", "#f8e45c", "#51a1ff", "#c061cb", "#4fd2fd", "#f6f5f4",
    ];
    if dark {
        ("#ffffff", "#1c1c1f", DARK)
    } else {
        ("#1d1d20", "#ffffff", LIGHT)
    }
}
