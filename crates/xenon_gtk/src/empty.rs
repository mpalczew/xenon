//! Stock-widget stand-ins: the empty workspace and tabs the GTK frontend
//! cannot draw yet.

use adw::prelude::*;
use xenon_core::TabState;

/// The Xenon icon, as the GPUI empty state shows it.
fn xenon_icon() -> Option<gtk::gdk::Texture> {
    let bytes =
        gtk::glib::Bytes::from_static(include_bytes!("../../../assets/xenon-icon-source.png"));
    gtk::gdk::Texture::from_bytes(&bytes).ok()
}

/// Commands the card lists: label and keys exactly as the GPUI catalog
/// (`xenon_ui/src/commands.rs`) writes them; `display_keys` turns the glyphs
/// into what this platform and the user's keymap bind.
const WITH_WORKSPACE: [(&str, &str); 5] = [
    ("Command Palette\u{2026}", "\u{2318}\u{21e7}P"),
    ("Focus Terminal", "\u{2318}1"),
    ("Focus Editor", "\u{2318}2"),
    ("Toggle Sidebar", "\u{2318}B"),
    ("Keyboard Shortcuts", "\u{2318}\u{21e7}/"),
];
const WITHOUT_WORKSPACE: [(&str, &str); 5] = [
    ("New Workspace\u{2026}", "\u{2318}\u{2325}N"),
    ("Open Workspace\u{2026}", "\u{2318}\u{21e7}O"),
    ("Command Palette\u{2026}", "\u{2318}\u{21e7}P"),
    ("Toggle Sidebar", "\u{2318}B"),
    ("Keyboard Shortcuts", "\u{2318}\u{21e7}/"),
];

fn button(label: &str, action: &str, tip: Option<&str>, primary: bool) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.set_action_name(Some(action));
    if primary {
        b.add_css_class("suggested-action");
    }
    b.set_tooltip_text(tip);
    b
}

/// The empty state, in the GPUI app's words and order (`xenon_ui/src/app/
/// empty_state.rs`): a workspace with nothing open in a pane, or no workspace.
/// An `AdwStatusPage` carries the icon, title and subtitle.
pub(crate) fn all_clear(_app: &crate::app::App, has_workspace: bool) -> gtk::Widget {
    let page = adw::StatusPage::new();
    page.set_paintable(xenon_icon().as_ref());
    page.set_title(if has_workspace {
        "Your workspace is ready"
    } else {
        "Open a workspace to get started"
    });
    page.set_description(Some(if has_workspace {
        "Start a terminal or open a file. Your surfaces will appear here."
    } else {
        "Xenon keeps your agent work organized across workspaces."
    }));
    page.set_vexpand(true);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 12);
    card.set_halign(gtk::Align::Center);
    card.set_size_request(420, -1);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::Center);
    buttons.set_margin_top(4);
    let tip = |t: &str| xenon_keymap::display_text(t);
    if has_workspace {
        buttons.append(&button(
            "New Terminal",
            "app.new-terminal",
            Some(&tip("New Terminal \u{b7} \u{2318}N")),
            true,
        ));
        buttons.append(&button(
            "Open File",
            "app.open-finder",
            Some(&tip("Open File \u{b7} \u{2318}O")),
            false,
        ));
        buttons.append(&button(
            "Close Workspace",
            "app.close-workspace",
            Some(&tip("Close Workspace \u{b7} \u{2318}\u{2325}W")),
            false,
        ));
    } else {
        buttons.append(&button(
            &tip("Open Workspace  \u{2318}\u{21e7}O"),
            "app.open-workspace",
            None,
            true,
        ));
    }
    card.append(&buttons);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.add_css_class("xhints");
    let heading = gtk::Label::new(Some("Useful shortcuts"));
    heading.add_css_class("xhints-head");
    heading.set_xalign(0.0);
    list.append(&heading);
    let entries = if has_workspace {
        &WITH_WORKSPACE
    } else {
        &WITHOUT_WORKSPACE
    };
    for (label, keys) in entries {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let name = gtk::Label::new(Some(label));
        name.add_css_class("xhints-name");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        let chord = gtk::Label::new(Some(&xenon_keymap::display_keys(keys)));
        chord.add_css_class("xhints-keys");
        row.append(&name);
        row.append(&chord);
        list.append(&row);
    }
    card.append(&list);
    page.set_child(Some(&card));
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
