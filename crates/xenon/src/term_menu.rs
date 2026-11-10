//! VTE leaves the click to a program that grabs the mouse (Shift bypasses
//! that) and opens the menu from the Menu key and Shift+F10 too. A press

use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio};
use vte::prelude::*;

use crate::links::{Link, classify};
use crate::term::TermView;

pub(crate) fn install(view: &Rc<TermView>) {
    view.term.set_context_menu_model(Some(&model(None)));
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_SECONDARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(view);
    // sync: runs before VTE's own handler, which reads the menu model right after.
    click.connect_pressed(move |_, _, x, y| {
        if let Some(view) = weak.upgrade() {
            view.term
                .set_context_menu_model(Some(&model(path_at(&view, x, y))));
        }
    });
    view.term.add_controller(click);
}

fn path_at(view: &TermView, x: f64, y: f64) -> Option<PathBuf> {
    let text = view.term.check_match_at(x, y).0?;
    match classify(&text, &view.live_cwd()) {
        Link::Path(path, _) if path.is_file() => Some(path),
        _ => None,
    }
}

fn item(label: &str, action: &str, target: Option<&PathBuf>) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    let target = target.map(|path| path.to_string_lossy().to_variant());
    item.set_action_and_target_value(Some(action), target.as_ref());
    item
}

fn model(path: Option<PathBuf>) -> gio::Menu {
    let menu = gio::Menu::new();
    if let Some(path) = &path {
        menu.append_item(&item("Open in Editor", "app.open-path-editor", Some(path)));
        menu.append_item(&item(
            "Open in Default App",
            "app.open-path-default",
            Some(path),
        ));
    }
    menu.append_item(&item("Cut", "app.copy", None));
    menu.append_item(&item("Copy", "app.copy", None));
    menu.append_item(&item("Copy Clean", "app.copy-clean", None));
    menu.append_item(&item("Copy Code", "app.copy-code", None));
    menu.append_item(&item("Paste", "app.paste", None));
    menu
}
