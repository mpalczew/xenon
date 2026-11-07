//! Settings for agents and scripts: the `settings-state` action publishes what
//! the window and the dialog look like (JSON), and `settings-page` and
//! `settings-set` drive the dialog through the same handlers its widgets use.
//! `scripts/xenon-gtk-verify` and `scripts/xenon-gtk-idle` read and drive these.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use vte::prelude::*;

use crate::app::App;

type Handler = Rc<dyn Fn(&str)>;

thread_local! {
    static HANDLES: RefCell<HashMap<&'static str, Handler>> = RefCell::new(HashMap::new());
}

/// Let `settings-set id=value` reach a widget's own change handler.
pub(crate) fn register(id: &'static str, handler: impl Fn(&str) + 'static) {
    HANDLES.with(|h| h.borrow_mut().insert(id, Rc::new(handler)));
}

pub(crate) fn forget_all() {
    HANDLES.with(|h| h.borrow_mut().clear());
}

impl App {
    /// Publish the settings picture as the `settings-state` action state.
    pub(crate) fn publish_settings(&self) {
        let Some(action) = self
            .gtk
            .lookup_action("settings-state")
            .and_downcast::<gio::SimpleAction>()
        else {
            return;
        };
        let dialog = crate::prefs::open_dialog();
        let term = self.current_term();
        // `has_focus` is false while the window is not the active one (a compositor
        // may refuse activation), so the window's focus child counts too.
        let window_focus = gtk::prelude::RootExt::focus(&self.win);
        let focus = if term.as_ref().is_some_and(|t| {
            t.term.has_focus()
                || window_focus
                    .as_ref()
                    .is_some_and(|f| f == t.term.upcast_ref::<gtk::Widget>())
        }) {
            "terminal"
        } else if window_focus.is_some() {
            "other"
        } else {
            "none"
        };
        let live = self.settings.borrow();
        let json = serde_json::json!({
            "open": dialog.is_some(),
            "page": dialog.as_ref().and_then(|d| d.visible_page_name()).map(|n| n.to_string()),
            "dark": adw::StyleManager::default().is_dark(),
            "preview": crate::prefs_apply::previewing().map(|(n, _)| n),
            "focus": focus,
            "term_font": term.as_ref().and_then(|t| t.term.font_desc()).map(|f| f.to_string()),
            "term_bg": term.as_ref().map(|t| t.term.color_background_for_draw().to_string()),
            "ui_size": live.ui.size,
            "terminal_size": live.terminal.size,
            "mode": format!("{:?}", live.theme.mode),
            "dark_theme": live.theme.dark,
            "light_theme": live.theme.light,
        })
        .to_string();
        drop(live);
        if action.state().and_then(|s| s.get::<String>()).as_deref() != Some(json.as_str()) {
            action.set_state(&json.to_variant());
        }
    }
}

pub(crate) fn install(app: &Rc<App>) {
    app.gtk.add_action(&gio::SimpleAction::new_stateful(
        "settings-state",
        None,
        &"".to_variant(),
    ));
    let page = gio::SimpleAction::new("settings-page", Some(glib::VariantTy::STRING));
    page.connect_activate(|_, param| {
        if let (Some(dialog), Some(name)) = (
            crate::prefs::open_dialog(),
            param.and_then(|p| p.str().map(String::from)),
        ) {
            dialog.set_visible_page_name(&name);
        }
    });
    app.gtk.add_action(&page);
    let set = gio::SimpleAction::new("settings-set", Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(app);
    set.connect_activate(move |_, param| {
        let Some((id, value)) = param
            .and_then(|p| p.str())
            .and_then(|t| t.split_once('='))
            .map(|(a, b)| (a.to_string(), b.to_string()))
        else {
            return;
        };
        let handler = HANDLES.with(|h| h.borrow().get(id.as_str()).cloned());
        match handler {
            Some(handler) => handler(&value),
            None => eprintln!("[xg] settings-set: no control {id:?}"),
        }
        if let Some(app) = weak.upgrade() {
            app.publish_settings();
        }
    });
    app.gtk.add_action(&set);
    let weak = Rc::downgrade(app);
    app.win.connect_focus_widget_notify(move |_| {
        if let Some(app) = weak.upgrade() {
            app.publish_settings();
        }
    });
    app.publish_settings();
}
