use adw::prelude::*;

use crate::commands::Cmd;

fn xenon_icon() -> Option<gtk::gdk::Texture> {
    let bytes =
        gtk::glib::Bytes::from_static(include_bytes!("../../../assets/xenon-icon-source.png"));
    gtk::gdk::Texture::from_bytes(&bytes).ok()
}

const WITH_WORKSPACE: [Cmd; 5] = [
    Cmd::CommandPalette,
    Cmd::OpenFinder,
    Cmd::OpenWorklist,
    Cmd::FocusTerminal,
    Cmd::ToggleSidebar,
];
const WITHOUT_WORKSPACE: [Cmd; 4] = [
    Cmd::NewWorkspace,
    Cmd::OpenWorkspace,
    Cmd::CommandPalette,
    Cmd::ToggleSidebar,
];

fn button(cmd: Cmd, primary: bool) -> gtk::Button {
    let b = gtk::Button::with_label(cmd.short());
    cmd.bind(&b);
    b.add_css_class("pill");
    if primary {
        b.add_css_class("suggested-action");
    }
    b
}

pub(crate) fn all_clear(has_workspace: bool) -> gtk::Widget {
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
        for (cmd, primary) in [
            (Cmd::NewTerminal, true),
            (Cmd::OpenFinder, false),
            (Cmd::OpenWorklist, false),
            (Cmd::CloseWorkspace, false),
        ] {
            buttons.append(&button(cmd, primary));
        }
    } else {
        buttons.append(&button(Cmd::OpenWorkspace, true));
    }
    card.append(&buttons);
    let heading = gtk::Label::new(Some("Useful shortcuts"));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    let entries: &[Cmd] = if has_workspace {
        &WITH_WORKSPACE
    } else {
        &WITHOUT_WORKSPACE
    };
    for cmd in entries {
        let row = adw::ActionRow::new();
        row.set_title(cmd.label());
        if let Some(chord) = cmd.chord() {
            let keys = gtk::Label::new(Some(&chord));
            keys.add_css_class("dim-label");
            row.add_suffix(&keys);
        }
        row.set_action_name(Some(cmd.action()));
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
