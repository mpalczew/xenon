use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::app::{App, defer};

impl App {
    pub(crate) fn close_hint_pending(&self) -> bool {
        !xenon_store::load_settings().is_ok_and(|s| s.close_hint_shown)
    }

    pub(crate) fn quit(self: &Rc<Self>) {
        if !self.close_hint_pending() {
            self.leave();
            return;
        }
        if let Err(error) = xenon_store::update_settings(|s| s.close_hint_shown = true) {
            eprintln!("[xg] save close hint: {error}");
        }
        let toast = adw::Toast::new("Closing the window leaves your terminals running");
        toast.set_timeout(2);
        self.toasts.add_toast(toast);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(std::time::Duration::from_millis(2200), move || {
            if let Some(app) = weak.upgrade() {
                app.leave();
            }
        });
    }

    pub(crate) fn leave(self: &Rc<Self>) {
        self.save_all();
        self.save_window();
        self.gtk.quit();
    }

    pub(crate) fn quit_end_terminals(self: &Rc<Self>) {
        let count = self.holders.list().len();
        if count == 0 {
            self.end_terminals_now();
            return;
        }
        let noun = if count == 1 { "terminal" } else { "terminals" };
        let dialog = adw::AlertDialog::new(
            Some("Quit and End Terminals?"),
            Some(&format!(
                "This ends {count} running {noun}, with any agents in them, on every build."
            )),
        );
        dialog.add_responses(&[("cancel", "Cancel"), ("end", "End Terminals")]);
        dialog.set_response_appearance("end", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response == "end" {
                defer(&weak, |app| app.end_terminals_now());
            }
        });
        dialog.present(Some(&self.win));
    }

    pub(crate) fn end_terminals_now(self: &Rc<Self>) {
        if let Err(error) = xenon_term::end_all() {
            eprintln!("[xg] end terminals: {error}");
        }
        self.leave();
    }
}
