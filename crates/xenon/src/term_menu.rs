use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio};
use vte::prelude::*;

use crate::commands::Cmd;
use crate::links::{Link, classify};
use crate::term::TermView;

pub(crate) fn install(view: &Rc<TermView>) {
    view.term.set_context_menu_model(Some(&model(None)));
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_SECONDARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(view);
    // sync: answer or ordering before return: runs before VTE's own handler, which reads the menu model right after.
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

fn model(path: Option<PathBuf>) -> gio::Menu {
    let menu = gio::Menu::new();
    if let Some(path) = &path {
        menu.append_item(&Cmd::OpenPathEditor.menu_item_for(path));
        menu.append_item(&Cmd::OpenPathDefault.menu_item_for(path));
    }
    for cmd in [
        Cmd::Cut,
        Cmd::Copy,
        Cmd::CopyClean,
        Cmd::CopyCode,
        Cmd::Paste,
    ] {
        menu.append_item(&cmd.menu_item());
    }
    menu
}
