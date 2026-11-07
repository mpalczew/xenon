//! The Settings window (Ctrl+,): an `AdwPreferencesDialog` with the same pages
//! as the GPUI app. Ctrl+1 to Ctrl+7 jump to a page, Ctrl+F searches, Escape
//! closes (or backs out of the theme picker first), and focus returns to where
//! it was. `app.settings` toggles it.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::app::App;
use crate::{prefs_agents, prefs_pages, prefs_picker, prefs_theme};

const PAGES: [&str; 7] = [
    "appearance",
    "fonts",
    "editor",
    "terminal",
    "agents",
    "servers",
    "phone",
];

thread_local! {
    static OPEN: RefCell<Option<adw::PreferencesDialog>> = const { RefCell::new(None) };
}

/// The Settings dialog while it is open.
pub(crate) fn open_dialog() -> Option<adw::PreferencesDialog> {
    OPEN.with(|o| o.borrow().clone())
}

impl App {
    /// Open Settings, or close it when it is already open.
    pub(crate) fn toggle_settings(self: &Rc<Self>) {
        if let Some(dialog) = open_dialog() {
            dialog.close();
            return;
        }
        self.launcher.close();
        let dialog = adw::PreferencesDialog::new();
        dialog.set_title("Settings");
        dialog.set_search_enabled(true);
        dialog.set_content_width(760);
        dialog.set_content_height(620);
        dialog.add(&prefs_theme::page(self, &dialog));
        dialog.add(&prefs_pages::fonts(self));
        dialog.add(&prefs_pages::editor(self));
        dialog.add(&prefs_pages::terminal(self));
        dialog.add(&prefs_agents::agents(&dialog));
        dialog.add(&prefs_agents::servers());
        dialog.add(&crate::prefs_phone::page(self));
        install_page_keys(&dialog);
        // One sheet paints every tile in its own theme's colors.
        let tiles = gtk::CssProvider::new();
        tiles.load_from_string(&prefs_picker::tile_css(&prefs_theme::tile_names(self)));
        let display = gtk::prelude::WidgetExt::display(&self.win);
        gtk::style_context_add_provider_for_display(
            &display,
            &tiles,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        let before = gtk::prelude::RootExt::focus(&self.win).map(|w| w.downgrade());
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            gtk::style_context_remove_provider_for_display(&display, &tiles);
            OPEN.with(|o| *o.borrow_mut() = None);
            crate::prefs_state::forget_all();
            let Some(app) = weak.upgrade() else { return };
            *app.remote.on_change.borrow_mut() = None;
            app.end_preview();
            match before.as_ref().and_then(|w| w.upgrade()) {
                Some(widget) if widget.is_mapped() => {
                    widget.grab_focus();
                }
                _ => app.focus_terminal(),
            }
            // Focus settles on the next idle; publish after it.
            let weak = Rc::downgrade(&app);
            glib::idle_add_local_once(move || {
                if let Some(app) = weak.upgrade() {
                    app.publish_settings();
                }
            });
        });
        {
            let weak = Rc::downgrade(self);
            dialog.connect_visible_page_name_notify(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.publish_settings();
                }
            });
            let d = dialog.clone();
            crate::prefs_state::register("close", move |_| {
                d.close();
            });
        }
        OPEN.with(|o| *o.borrow_mut() = Some(dialog.clone()));
        dialog.present(Some(&self.win));
        self.publish_settings();
    }
}

fn install_page_keys(dialog: &adw::PreferencesDialog) {
    let controller = gtk::ShortcutController::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    for (index, name) in PAGES.iter().enumerate() {
        let weak = dialog.downgrade();
        let action = gtk::CallbackAction::new(move |_, _| {
            if let Some(dialog) = weak.upgrade() {
                dialog.set_visible_page_name(name);
            }
            glib::Propagation::Stop
        });
        if let Some(trigger) =
            gtk::ShortcutTrigger::parse_string(&format!("<Control>{}", index + 1))
        {
            controller.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
        }
    }
    dialog.add_controller(controller);
}
