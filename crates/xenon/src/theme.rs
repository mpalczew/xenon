pub(crate) const CSS: &str = "\
listview.xfiles > row { min-height: 24px; padding: 0 6px; }
listview.xfiles treeexpander:drop(active) { background: alpha(@accent_color, 0.25); }
listview.xfiles:drop(active) { box-shadow: inset 0 0 0 2px @accent_color; }
vte-terminal { padding: 4px 8px; }
listbox.xworkspaces:drop(active) { box-shadow: none; }
listbox.xworkspaces > row.xdragging { opacity: 0.45; }
.xdrop-line, .xdrop-cap { background: @accent_color; border-radius: 99px; }
.xdrop-line { min-height: 2px; }
.xdrop-cap { min-width: 6px; min-height: 6px; }
.xdrag-ghost { background: @view_bg_color; border-radius: 99px; padding: 3px 10px; font-weight: bold; box-shadow: 0 1px 3px alpha(black, 0.18), 0 6px 16px alpha(black, 0.14); }
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

pub(crate) fn scheme(dark: bool) -> Option<sourceview::StyleScheme> {
    sourceview::StyleSchemeManager::default().scheme(if dark { "Adwaita-dark" } else { "Adwaita" })
}

pub(crate) fn dark() -> bool {
    adw::StyleManager::default().is_dark()
}

/// Terminal foreground, background and the 16 ANSI colors for light or dark.
/// Both come from the default "GNOME" palette of Ptyxis (GPL-3.0-or-later),
/// its Light and Dark variants, with that palette's own foreground and
/// background.
pub(crate) fn terminal(dark: bool) -> (&'static str, &'static str, [&'static str; 16]) {
    const LIGHT: [&str; 16] = [
        "#1d1d20", "#c01c28", "#26a269", "#a2734c", "#12488b", "#a347ba", "#2aa1b3", "#767676",
        "#5d5d5d", "#f66151", "#29a963", "#bd8d0a", "#2a7bde", "#c061cb", "#2aa2b5", "#949494",
    ];
    const DARK: [&str; 16] = [
        "#696672", "#c42934", "#2ec27e", "#f5c211", "#1e78e4", "#9841bb", "#0ab9dc", "#c0bfbc",
        "#848389", "#ed333b", "#57e389", "#f8e45c", "#51a1ff", "#c061cb", "#4fd2fd", "#f6f5f4",
    ];
    if dark {
        ("#ffffff", "#1c1c1f", DARK)
    } else {
        ("#1d1d20", "#ffffff", LIGHT)
    }
}
