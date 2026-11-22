use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::app::App;
use crate::palette::{Handlers, Options, PaletteRow};
use crate::picker::hits;

const COMMANDS: &[(&str, &str)] = &[
    ("New Terminal", "app.new-terminal"),
    ("New Workspace\u{2026}", "app.new-workspace"),
    ("Open Workspace\u{2026}", "app.open-workspace"),
    ("Go to File\u{2026}", "app.open-finder"),
    ("Run Task\u{2026}", "app.run-task"),
    ("Run Build Task", "app.run-build-task"),
    ("Add to Worklist", "app.capture-worklist"),
    ("Open Worklist", "app.open-worklist"),
    ("Undo Worklist Capture", "app.undo-capture"),
    ("Open in Browser", "app.open-in-browser"),
    ("Close Tab", "app.close-tab"),
    ("Close Other Tabs", "app.close-other-tabs"),
    ("Next Tab", "app.next-tab"),
    ("Previous Tab", "app.prev-tab"),
    ("Next Pane", "app.next-pane"),
    ("Next Workspace", "app.next-workspace"),
    ("Previous Workspace", "app.prev-workspace"),
    ("Move Workspace Up", "app.move-workspace-up"),
    ("Move Workspace Down", "app.move-workspace-down"),
    ("Close Workspace", "app.close-workspace"),
    ("Split Right", "app.split-right"),
    ("Split Down", "app.split-down"),
    ("Reserve Pane to the Right", "app.park-pane"),
    ("Focus Terminal", "app.focus-terminal"),
    ("Focus Sidebar", "app.focus-sidebar"),
    ("Focus Git", "app.focus-git"),
    ("Toggle Sidebar", "app.toggle-sidebar"),
    ("Toggle Files", "app.toggle-files"),
    ("Show Hidden Files", "app.show-hidden-files"),
    ("Cycle Diff Layout", "app.cycle-diff-layout"),
    ("Find in Terminal", "app.find"),
    ("Find Next", "app.find-next"),
    ("Find Previous", "app.find-prev"),
    ("Cut", "app.cut"),
    ("Copy", "app.copy"),
    ("Copy Clean", "app.copy-clean"),
    ("Copy Code", "app.copy-code"),
    ("Paste", "app.paste"),
    ("Zoom In", "app.zoom-in"),
    ("Zoom Out", "app.zoom-out"),
    ("Reset Zoom", "app.zoom-reset"),
    ("Save", "app.save"),
    ("Save All", "app.save-now"),
    ("Settings", "app.settings"),
    ("Quit", "app.quit"),
    ("Quit and End Terminals", "app.quit-end-terminals"),
];

fn filter(query: &str) -> Vec<(&'static str, &'static str)> {
    if query.is_empty() {
        return COMMANDS.to_vec();
    }
    crate::picker::rank(COMMANDS, query, |(title, _)| (*title).to_string())
        .into_iter()
        .map(|i| COMMANDS[i])
        .collect()
}

impl App {
    pub(crate) fn open_command_palette(self: &Rc<Self>) {
        let shown: Rc<std::cell::RefCell<Vec<&'static str>>> = Rc::default();
        let (weak_q, weak_a) = (Rc::downgrade(self), Rc::downgrade(self));
        let (shown_q, shown_a) = (shown.clone(), shown);
        self.launcher.open(
            &self.win,
            Handlers {
                placeholder: "Search commands\u{2026}".into(),
                hint: "return runs  \u{b7}  esc closes".into(),
                options: Options::default(),
                on_query: Box::new(move |query| {
                    let Some(app) = weak_q.upgrade() else { return };
                    let found = filter(query);
                    let rows = found
                        .iter()
                        .map(|(title, action)| PaletteRow {
                            title: (*title).into(),
                            hits: hits(title, query),
                            badge: app.chord(action),
                            enabled: true,
                            ..PaletteRow::default()
                        })
                        .collect();
                    *shown_q.borrow_mut() = found.iter().map(|(_, a)| *a).collect();
                    app.launcher.set_rows(rows, "No matching commands");
                }),
                on_accept: Box::new(move |row| {
                    let Some(app) = weak_a.upgrade() else { return };
                    let Some(action) = shown_a.borrow().get(row).copied() else {
                        return;
                    };
                    app.launcher.close();
                    glib::idle_add_local_once(move || {
                        app.gtk.activate_action(&action["app.".len()..], None);
                    });
                }),
                on_complete: None,
            },
        );
    }
}
