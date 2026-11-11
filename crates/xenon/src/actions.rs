use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use vte::prelude::*;
use xenon_core::SplitAxis;

use crate::app::{App, defer};
use crate::model::Fold;

type Handler = fn(&Rc<App>);

const SIMPLE: &[(&str, Handler)] = &[
    ("new-terminal", |a| a.new_terminal()),
    ("new-workspace", |a| a.new_workspace_dialog()),
    ("open-workspace", |a| a.open_workspace_picker()),
    ("browse-workspace", |a| a.open_workspace_dialog()),
    ("open-finder", |a| a.open_finder()),
    ("command-palette", |a| a.open_command_palette()),
    ("run-task", |a| a.open_task_picker()),
    ("capture-worklist", |a| a.capture_worklist()),
    ("open-worklist", |a| a.open_worklist()),
    ("undo-capture", |a| a.undo_capture()),
    ("run-build-task", |a| a.run_build_task()),
    ("allow-folder-tasks", |a| a.answer_folder_tasks(true)),
    ("deny-folder-tasks", |a| a.answer_folder_tasks(false)),
    ("open-in-browser", |a| a.open_in_browser()),
    ("close-tab", |a| a.close_active_tab()),
    ("close-other-tabs", |a| a.close_other_tabs()),
    ("next-tab", |a| a.step_tab(1)),
    ("prev-tab", |a| a.step_tab(-1)),
    ("next-pane", |a| a.next_pane()),
    ("next-workspace", |a| a.step_workspace(1)),
    ("prev-workspace", |a| a.step_workspace(-1)),
    ("move-workspace-up", |a| a.shift_workspace(-1)),
    ("move-workspace-down", |a| a.shift_workspace(1)),
    ("close-workspace", |a| a.close_workspace()),
    ("split-right", |a| a.split(SplitAxis::Horizontal)),
    ("split-down", |a| a.split(SplitAxis::Vertical)),
    ("park-pane", |a| a.park_pane()),
    ("focus-terminal", |a| a.focus_terminal()),
    ("focus-sidebar", |a| a.focus_sidebar()),
    ("focus-git", |a| a.focus_git()),
    ("toggle-files", |a| a.toggle_fold(Fold::Files)),
    ("cycle-diff-layout", |a| a.cycle_diff_layout()),
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
    ("cut", |a| a.cut()),
    ("copy-clean", |a| a.copy_clean()),
    ("copy-code", |a| a.copy_code()),
    ("paste", |a| a.paste()),
    ("zoom-in", |a| a.zoom(1.0)),
    ("zoom-out", |a| a.zoom(-1.0)),
    ("zoom-reset", |a| a.zoom(0.0)),
    ("save", |a| a.save_active_editor(a)),
    ("save-now", |a| a.save_all()),
    ("settings", |a| a.toggle_settings()),
    ("quit", |a| a.quit()),
];

impl App {
    pub(crate) fn quit(self: &Rc<Self>) {
        self.save_all();
        self.save_window();
        self.gtk.quit();
    }
}

pub(crate) fn install(app: &Rc<App>) {
    for (name, handler) in SIMPLE {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(app);
        let handler = *handler;
        let name = *name;
        action.connect_activate(move |_, _| {
            defer(&weak, move |app| {
                if app.launcher.is_open() && !palette_passes(name) {
                    match name {
                        "new-terminal" => app.launcher.key("down"),
                        "open-finder" => app.launcher.key("up"),
                        _ => {}
                    }
                    return;
                }
                handler(&app);
            });
        });
        app.gtk.add_action(&action);
    }
    install_hidden_files(app);
    install_term_actions(app);
    crate::remote_ui::install(app);
    install_palette_keys(app);
    crate::keys::install(&app.gtk);
}

/// A boolean action, so menus show a check. Activating it asks for the flipped state.
fn install_hidden_files(app: &Rc<App>) {
    let on = xenon_store::load_settings().is_ok_and(|s| s.show_hidden_files);
    let action = gio::SimpleAction::new_stateful("show-hidden-files", None, &on.to_variant());
    let weak = Rc::downgrade(app);
    // sync: event-time read: only reads the requested state; the change is deferred.
    action.connect_change_state(move |_, value| {
        let Some(on) = value.and_then(|v| v.get::<bool>()) else {
            return;
        };
        defer(&weak, move |app| app.sidebar.files.set_hidden(on));
    });
    app.gtk.add_action(&action);
}

fn palette_passes(name: &str) -> bool {
    matches!(name, "quit" | "save-now" | "open-workspace" | "settings")
}

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
            let text = text.to_owned();
            glib::idle_add_local_once(move || run(&app, &text));
        }
    });
    app.gtk.add_action(&action);
}

fn install_term_actions(app: &Rc<App>) {
    string_action(app, "open-path-editor", |app, path| {
        if let Some(ws) = app.model.borrow().active() {
            app.open_editor(ws, std::path::Path::new(path));
        }
    });
    string_action(app, "open-path-default", |app, path| {
        app.open_file(std::path::Path::new(path));
    });
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
            glib::idle_add_local_once(move || app.term_interaction(ws, tab));
        }
    });
    app.gtk.add_action(&local);
}
