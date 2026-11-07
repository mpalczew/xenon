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
    ("open-workspace", |a| a.open_workspace_picker()),
    ("browse-workspace", |a| a.open_workspace_dialog()),
    ("open-finder", |a| a.open_finder()),
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
    ("settings", |a| a.toggle_settings()),
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

    pub(crate) fn publish_palette(&self) {
        self.publish_to("palette-state", &self.launcher.snapshot());
    }

    pub(crate) fn publish_files(&self) {
        self.publish_to("files-state", &self.sidebar.files.snapshot());
    }

    fn publish_to(&self, name: &str, json: &str) {
        let Some(action) = self
            .gtk
            .lookup_action(name)
            .and_downcast::<gio::SimpleAction>()
        else {
            return;
        };
        if action.state().and_then(|s| s.get::<String>()).as_deref() != Some(json) {
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
        let name = *name;
        action.connect_activate(move |_, _| {
            if let Some(app) = weak.upgrade() {
                if app.launcher.is_open() && !palette_passes(name) {
                    // Ctrl+N / Ctrl+P move in the palette; the rest wait.
                    match name {
                        "new-terminal" => app.launcher.key("down"),
                        "open-finder" => app.launcher.key("up"),
                        _ => {}
                    }
                    return;
                }
                handler(&app);
            }
        });
        app.gtk.add_action(&action);
    }
    install_automation(app);
    crate::remote_ui::install(app);
    crate::prefs_state::install(app);
    for name in ["palette-state", "files-state"] {
        app.gtk.add_action(&gio::SimpleAction::new_stateful(
            name,
            None,
            &"".to_variant(),
        ));
    }
    install_palette_keys(app);
    let state = gio::SimpleAction::new_stateful("ui-state", None, &"".to_variant());
    app.gtk.add_action(&state);
    for (action, accels) in crate::keys::bindings(user_keys) {
        let refs: Vec<&str> = accels.iter().map(String::as_str).collect();
        app.gtk.set_accels_for_action(action, &refs);
    }
}

/// Actions that still run while the palette is open.
fn palette_passes(name: &str) -> bool {
    matches!(name, "quit" | "save-now" | "open-workspace" | "settings")
}

/// Ctrl+O outside a terminal opens the workspace picker too. The window
/// shortcut runs after the focused widget, so a terminal (or the Vim input
/// method in an editor) that takes the key keeps it.
fn install_palette_keys(app: &Rc<App>) {
    let controller = gtk::ShortcutController::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
    let weak = Rc::downgrade(app);
    let action = gtk::CallbackAction::new(move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.open_workspace_picker();
        }
        glib::Propagation::Stop
    });
    if let Some(trigger) = gtk::ShortcutTrigger::parse_string("<Control>o") {
        controller.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
    }
    app.win.add_controller(controller);
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
            crate::latency::input();
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
    // A key chord as the app maps it (`ctrl-n`, `ctrl-tab`): run the action the
    // accelerator is bound to, or, when nothing is bound (Ctrl+C and the other
    // keys the shell owns), type it into the terminal. Wayland has no portable
    // key injection, so this checks the key map without a real key press.
    string_action(app, "send-key", |app, keystroke| {
        let parsed = crate::keys::accelerator(keystroke).and_then(|a| gtk::accelerator_parse(&a));
        let Some((key, mods)) = parsed else {
            eprintln!("[xg] send-key: cannot parse {keystroke:?}");
            return;
        };
        let bound = app.gtk.actions_for_accel(&gtk::accelerator_name(key, mods));
        if let Some(action) = bound.first() {
            let _ = gtk::prelude::WidgetExt::activate_action(&app.win, action, None);
        } else if let Some(byte) = crate::keys::control_byte(keystroke)
            && let Some(view) = app.current_term()
        {
            view.term.feed_child(&[byte]);
        }
    });
    // What a key press in the terminal reports (the key controller calls the
    // same function): a Mac keystroke, which takes a phone's grid back.
    let local = gio::SimpleAction::new("term-interaction", None);
    let weak = Rc::downgrade(app);
    local.connect_activate(move |_, _| {
        let Some(app) = weak.upgrade() else { return };
        let key = {
            let model = app.model.borrow();
            let ws = model.active();
            let tab = ws
                .and_then(|w| model.spaces.get(&w))
                .and_then(|s| s.content.active_tab());
            ws.zip(tab.map(|t| t.id))
        };
        if let Some((ws, tab)) = key {
            app.term_interaction(ws, tab);
        }
    });
    app.gtk.add_action(&local);
    string_action(app, "palette-text", |app, text| app.launcher.set_text(text));
    string_action(app, "palette-key", |app, key| app.launcher.key(key));
    string_action(app, "files-key", |app, key| app.sidebar.files.key(key));
    string_action(app, "files-reveal", |app, path| {
        app.sidebar.files.reveal(std::path::Path::new(path));
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
