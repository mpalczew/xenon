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
.xdiff-note { padding: 14px 16px; }
.xdiff-half.right { border-left: 1px solid @borders; }
";

pub(crate) fn scheme(dark: bool) -> Option<sourceview::StyleScheme> {
    sourceview::StyleSchemeManager::default().scheme(if dark { "Adwaita-dark" } else { "Adwaita" })
}

pub(crate) fn dark() -> bool {
    adw::StyleManager::default().is_dark()
}
