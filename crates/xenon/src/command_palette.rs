use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::app::App;
use crate::commands::Cmd;
use crate::palette::{Handlers, Options, PaletteRow};
use crate::picker::hits;

fn filter(query: &str) -> Vec<Cmd> {
    let all: Vec<Cmd> = Cmd::palette().collect();
    crate::picker::rank(&all, query, |cmd| cmd.label().to_string())
        .into_iter()
        .map(|i| all[i])
        .collect()
}

impl App {
    pub(crate) fn open_command_palette(self: &Rc<Self>) {
        let shown: Rc<std::cell::RefCell<Vec<Cmd>>> = Rc::default();
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
                        .map(|cmd| PaletteRow {
                            title: cmd.label().into(),
                            hits: hits(cmd.label(), query),
                            badge: cmd.chord(),
                            enabled: true,
                            ..PaletteRow::default()
                        })
                        .collect();
                    *shown_q.borrow_mut() = found.clone();
                    app.launcher.set_rows(rows, "No matching commands");
                }),
                on_accept: Box::new(move |row| {
                    let Some(app) = weak_a.upgrade() else { return };
                    let Some(cmd) = shown_a.borrow().get(row).copied() else {
                        return;
                    };
                    app.launcher.close();
                    glib::idle_add_local_once(move || {
                        app.gtk.activate_action(cmd.name(), None);
                    });
                }),
                on_complete: None,
            },
        );
    }
}
