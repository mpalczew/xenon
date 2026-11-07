//! The look is the desktop's: stock libadwaita colors, backgrounds and accent,
//! following light and dark through `adw::StyleManager`. What is left is the
//! static sheet for our own widgets and two small light or dark switches, the
//! terminal palette here and the GtkSourceView scheme (`scheme`).

/// Rules for widgets of ours that have no stock equivalent. Colors are
/// libadwaita's named colors or the text color at low strength.
pub(crate) const CSS: &str = "\
vte-terminal { padding: 4px 8px; }
.xsidebar row.drop-before { box-shadow: 0 -1px 0 1px @accent_color; }
.xsidebar row.drop-after { box-shadow: 0 1px 0 1px @accent_color; }
.xdot { min-width: 3px; border-radius: 2px; margin: 2px 6px 2px 0; }
.xdot.working { background: @accent_color; }
.xdot.attention { background: @warning_color; }
.xfiles row.xopen label { color: @accent_color; }
.xdrop { background: alpha(@accent_color, 0.35); border: 2px solid @accent_color; }
.xpane { border-top: 2px solid transparent; }
.xpane:focus-within:not(:only-child) { border-top-color: @accent_color; }
.xfind { border-bottom: 1px solid @borders; padding: 4px 8px; }
.xgutter { min-width: 20px; margin-right: 0; }
.xrow-close { min-width: 20px; min-height: 20px; padding: 0; opacity: 0; }
row:hover .xrow-close, .xrow-close:focus { opacity: 0.8; }
row:hover .xtick { opacity: 0; }
.xhints { border-top: 1px solid @borders; padding-top: 12px; margin-top: 4px; }
.xtick { min-width: 5px; min-height: 18px; border-radius: 3px; }
.xtick.dirty { background: @warning_color; }
.xtick.index { border: 2px solid @warning_color; }
.xgit-section { border-top: 1px solid @borders; }
.xdeleted { text-decoration: line-through; }
.xbadge { min-width: 18px; min-height: 18px; border-radius: 9px; padding: 0; }
.xbadge.xk-modified { background: alpha(@warning_color, 0.18); }
.xbadge.xk-added { background: alpha(@success_color, 0.18); }
.xbadge.xk-deleted { background: alpha(@error_color, 0.18); }
.xbadge.xk-renamed { background: alpha(@accent_color, 0.18); }
.xbadge.xk-conflict { background: alpha(@purple_3, 0.18); }
.xbadge.xk-untracked { background: alpha(currentColor, 0.1); }
.xk-modified { color: @warning_color; }
.xk-added { color: @success_color; }
.xk-deleted { color: @error_color; }
.xk-renamed { color: @accent_color; }
.xk-conflict { color: @purple_3; }
.xcell { min-width: 4px; min-height: 10px; border-radius: 1px; }
.xcell.add { background: @success_color; }
.xcell.del { background: @error_color; }
.xcell.neutral { background: alpha(currentColor, 0.3); }
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
