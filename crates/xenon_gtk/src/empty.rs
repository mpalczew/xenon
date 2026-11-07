//! Stock-widget stand-ins: the empty workspace and tabs the GTK frontend
//! cannot draw yet.

use adw::prelude::*;
use xenon_core::TabState;

/// A workspace with no tabs.
pub(crate) fn all_clear() -> gtk::Widget {
    let page = adw::StatusPage::new();
    page.set_icon_name(Some("utilities-terminal-symbolic"));
    page.set_title("All clear");
    page.set_description(Some(
        "Ctrl+N opens a terminal. Ctrl+Shift+O opens a workspace.",
    ));
    page.set_vexpand(true);
    page.upcast()
}

/// An editor tab: kept as saved, shown as a note.
pub(crate) fn placeholder(state: &TabState) -> gtk::Widget {
    let (title, detail) = match state {
        TabState::Editor { path, .. } => (
            path.file_name()
                .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned()),
            path.display().to_string(),
        ),
        TabState::Terminal { .. } => ("terminal".into(), String::new()),
    };
    let page = adw::StatusPage::new();
    page.add_css_class("xplaceholder");
    page.set_icon_name(Some("text-x-generic-symbolic"));
    page.set_title(&title);
    page.set_description(Some(&format!(
        "{detail}\nEditor tabs arrive in a later step. This tab is kept as saved."
    )));
    page.upcast()
}
