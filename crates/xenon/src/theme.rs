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
/// The ANSI colors are the Tango palette (public domain, the Tango Desktop
/// Project); the defaults follow Adwaita's window colors.
pub(crate) fn terminal(dark: bool) -> (&'static str, &'static str, [&'static str; 16]) {
    const LIGHT: [&str; 16] = [
        "#2e3436", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a", "#d3d7cf",
        "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8", "#34e2e2", "#eeeeec",
    ];
    if dark {
        ("#ffffff", "#242424", LIGHT)
    } else {
        ("#2e3436", "#ffffff", LIGHT)
    }
}
