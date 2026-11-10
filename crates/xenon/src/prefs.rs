use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::app::App;
use crate::{prefs_agents, prefs_pages};

thread_local! {
    static OPEN: RefCell<Option<adw::PreferencesDialog>> = const { RefCell::new(None) };
}

pub(crate) fn open_dialog() -> Option<adw::PreferencesDialog> {
    OPEN.with(|o| o.borrow().clone())
}

impl App {
    pub(crate) fn toggle_settings(self: &Rc<Self>) {
        if let Some(dialog) = open_dialog() {
            dialog.present(Some(&self.win));
            return;
        }
        self.launcher.close();
        let dialog = adw::PreferencesDialog::builder()
            .title("Settings")
            .content_width(860)
            .content_height(640)
            .build();
        dialog.add(&prefs_pages::appearance(self));
        dialog.add(&prefs_pages::fonts(self));
        dialog.add(&prefs_pages::editor(self));
        let agents = prefs_agents::agents(&dialog);
        agents.add(&prefs_pages::terminal(self));
        dialog.add(&agents);
        dialog.add(&crate::prefs_phone::page(self));
        let before = gtk::prelude::RootExt::focus(&self.win).map(|w| w.downgrade());
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            OPEN.with(|o| *o.borrow_mut() = None);
            let Some(app) = weak.upgrade() else { return };
            *app.remote.on_change.borrow_mut() = None;
            match before.as_ref().and_then(|w| w.upgrade()) {
                Some(widget) if widget.is_mapped() => {
                    glib::idle_add_local_once(move || {
                        widget.grab_focus();
                    });
                }
                _ => {
                    glib::idle_add_local_once(move || app.focus_terminal());
                }
            }
        });
        OPEN.with(|o| *o.borrow_mut() = Some(dialog.clone()));
        dialog.present(Some(&self.win));
    }
}
