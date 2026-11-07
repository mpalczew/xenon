//! GTK actions and their accelerators. Every command is an `app.*` action, so
//! a key, a menu item, and a D-Bus call all take the same path. The extra
//! actions at the bottom exist for agents and scripts: they type into the
//! terminal, open a folder, save, and render the window to a PNG.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use vte::prelude::*;
use xenon_core::SplitAxis;
use xenon_keymap::Entry;

use crate::app::App;

type Handler = fn(&Rc<App>);

const SIMPLE: &[(&str, Handler)] = &[
    ("new-terminal", |a| a.new_terminal()),
    ("new-workspace", |a| a.new_workspace_dialog()),
    ("open-workspace", |a| a.open_workspace_dialog()),
    ("close-tab", |a| a.close_active_tab()),
    ("close-other-tabs", |a| a.close_other_tabs()),
    ("next-tab", |a| a.step_tab(1)),
    ("prev-tab", |a| a.step_tab(-1)),
    ("next-pane", |a| a.next_pane()),
    ("next-workspace", |a| a.step_workspace(1)),
    ("prev-workspace", |a| a.step_workspace(-1)),
    ("close-workspace", |a| a.close_workspace()),
    ("split-right", |a| a.split(SplitAxis::Horizontal)),
    ("split-down", |a| a.split(SplitAxis::Vertical)),
    ("park-pane", |a| a.park_pane()),
    ("focus-terminal", |a| a.focus_terminal()),
    ("focus-sidebar", |a| a.focus_sidebar()),
    ("toggle-sidebar", |a| a.toggle_sidebar()),
    ("find", |a| {
        if let Some(view) = a.current_term() {
            view.find.open();
        }
    }),
    ("find-next", |a| {
        if let Some(view) = a.current_term() {
            view.term.search_find_next();
        }
    }),
    ("find-prev", |a| {
        if let Some(view) = a.current_term() {
            view.term.search_find_previous();
        }
    }),
    ("copy", |a| {
        if let Some(view) = a.current_term() {
            view.term.copy_clipboard_format(vte::Format::Text);
        }
    }),
    ("paste", |a| {
        if let Some(view) = a.current_term() {
            view.term.paste_clipboard();
        }
    }),
    ("zoom-in", |a| a.zoom(1.0)),
    ("zoom-out", |a| a.zoom(-1.0)),
    ("zoom-reset", |a| a.zoom(0.0)),
    ("save-now", |a| a.save_all()),
    ("quit", |a| a.quit()),
];

impl App {
    /// Publish the model's picture as the `ui-state` action state.
    pub(crate) fn publish_state(&self) {
        let json = self.model.borrow().describe();
        let Some(action) = self
            .gtk
            .lookup_action("ui-state")
            .and_downcast::<gio::SimpleAction>()
        else {
            return;
        };
        if action.state().and_then(|s| s.get::<String>()).as_deref() != Some(json.as_str()) {
            action.set_state(&json.to_variant());
        }
    }

    pub(crate) fn quit(self: &Rc<Self>) {
        self.save_all();
        self.save_window();
        self.gtk.quit();
    }
}

pub(crate) fn install(app: &Rc<App>, user_keys: &[Entry]) {
    for (name, handler) in SIMPLE {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(app);
        let handler = *handler;
        action.connect_activate(move |_, _| {
            if let Some(app) = weak.upgrade() {
                handler(&app);
            }
        });
        app.gtk.add_action(&action);
    }
    install_automation(app);
    let state = gio::SimpleAction::new_stateful("ui-state", None, &"".to_variant());
    app.gtk.add_action(&state);
    for (action, accels) in crate::keys::bindings(user_keys) {
        let refs: Vec<&str> = accels.iter().map(String::as_str).collect();
        app.gtk.set_accels_for_action(action, &refs);
    }
}

fn string_action(app: &Rc<App>, name: &str, run: fn(&Rc<App>, &str)) {
    let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(app);
    action.connect_activate(move |_, param| {
        if let (Some(app), Some(text)) = (weak.upgrade(), param.and_then(|p| p.str())) {
            run(&app, text);
        }
    });
    app.gtk.add_action(&action);
}

fn install_automation(app: &Rc<App>) {
    string_action(app, "feed-child", |app, text| {
        if let Some(view) = app.current_term() {
            view.term.feed_child(text.as_bytes());
        }
    });
    // For displays without an accessibility bridge (Broadway): the visible
    // screen text of the current terminal, written to a file.
    string_action(app, "dump-screen", |app, path| {
        if let Some(view) = app.current_term() {
            let text = view.term.text_format(vte::Format::Text).unwrap_or_default();
            if let Err(error) = std::fs::write(path, text.as_str()) {
                eprintln!("[xg] dump-screen {path}: {error}");
            }
        }
    });
    string_action(app, "find-query", |app, query| {
        if let Some(view) = app.current_term() {
            view.find.open();
            view.find.entry.set_text(query);
        }
    });
    string_action(app, "open-link", |app, text| {
        let key = {
            let model = app.model.borrow();
            let ws = model.active();
            let tab = ws
                .and_then(|w| model.spaces.get(&w))
                .and_then(|s| s.content.active_tab());
            ws.zip(tab.map(|t| t.id))
        };
        if let Some((ws, tab)) = key {
            app.open_link(ws, tab, text);
        }
    });
    string_action(app, "open-dir", |app, path| {
        app.open_workspace(std::path::Path::new(path))
    });
    string_action(app, "screenshot", |app, path| {
        let name = path.to_string();
        crate::screenshot::render_png(&app.win, path, move |result| {
            if let Err(error) = result {
                eprintln!("[xg] screenshot {name}: {error}");
            }
        });
    });
}
