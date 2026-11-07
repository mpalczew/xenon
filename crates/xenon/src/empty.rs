//! Stock-widget stand-ins: the empty workspace and tabs the GTK frontend
//! cannot draw yet.

use adw::prelude::*;
use xenon_core::TabState;

/// The Xenon icon.
fn xenon_icon() -> Option<gtk::gdk::Texture> {
    let bytes =
        gtk::glib::Bytes::from_static(include_bytes!("../../../assets/xenon-icon-source.png"));
    gtk::gdk::Texture::from_bytes(&bytes).ok()
}

/// Commands the card lists: label and the GTK action the row runs. The keys
/// shown come from the key table.
type Entry = (&'static str, &'static str);
const WITH_WORKSPACE: [Entry; 4] = [
    ("Command Palette\u{2026}", "app.command-palette"),
    ("Go to File\u{2026}", "app.open-finder"),
    ("Focus Terminal", "app.focus-terminal"),
    ("Toggle Sidebar", "app.toggle-sidebar"),
];
const WITHOUT_WORKSPACE: [Entry; 4] = [
    ("New Workspace\u{2026}", "app.new-workspace"),
    ("Open Workspace\u{2026}", "app.open-workspace"),
    ("Command Palette\u{2026}", "app.command-palette"),
    ("Toggle Sidebar", "app.toggle-sidebar"),
];

fn button(label: &str, action: &str, tip: Option<&str>, primary: bool) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.set_action_name(Some(action));
    b.add_css_class("pill");
    if primary {
        b.add_css_class("suggested-action");
    }
    b.set_tooltip_text(tip);
    b
}

/// The empty state: a workspace with nothing open in a pane, or no workspace.
/// An `AdwStatusPage` carries the icon, title and subtitle.
pub(crate) fn all_clear(app: &crate::app::App, has_workspace: bool) -> gtk::Widget {
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
    let card = gtk::Box::new(gtk::Orientation::Vertical, 24);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::Center);
    if has_workspace {
        buttons.append(&button(
            "New Terminal",
            "app.new-terminal",
            Some(&app.tip("New Terminal", "app.new-terminal")),
            true,
        ));
        buttons.append(&button(
            "Open File",
            "app.open-finder",
            Some(&app.tip("Open File", "app.open-finder")),
            false,
        ));
        buttons.append(&button(
            "Close Workspace",
            "app.close-workspace",
            Some(&app.tip("Close Workspace", "app.close-workspace")),
            false,
        ));
    } else {
        buttons.append(&button(
            &match app.chord("app.open-workspace") {
                Some(chord) => format!("Open Workspace  {chord}"),
                None => "Open Workspace".to_string(),
            },
            "app.open-workspace",
            None,
            true,
        ));
    }
    card.append(&buttons);
    let heading = gtk::Label::new(Some("Useful shortcuts"));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    let entries = if has_workspace {
        &WITH_WORKSPACE
    } else {
        &WITHOUT_WORKSPACE
    };
    for (label, action) in entries {
        let row = adw::ActionRow::new();
        row.set_title(label);
        if let Some(chord) = app.chord(action) {
            let keys = gtk::Label::new(Some(&chord));
            keys.add_css_class("dim-label");
            row.add_suffix(&keys);
        }
        row.set_action_name(Some(action));
        list.append(&row);
    }
    let shortcuts = gtk::Box::new(gtk::Orientation::Vertical, 6);
    shortcuts.append(&heading);
    shortcuts.append(&list);
    card.append(&shortcuts);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(420);
    clamp.set_child(Some(&card));
    page.set_child(Some(&clamp));
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
        TabState::Diff { path, .. } => (path.clone(), String::new()),
        TabState::Terminal { .. } => ("terminal".into(), String::new()),
    };
    let page = adw::StatusPage::new();
    page.set_icon_name(Some("text-x-generic-symbolic"));
    page.set_title(&title);
    page.set_description(Some(&detail));
    page.upcast()
}
