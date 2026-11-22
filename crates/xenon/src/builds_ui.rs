use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use xenon_core::{TabId, TermId, WorkspaceId};
use xenon_term::TermInfo;

use crate::app::{App, defer};

pub(crate) struct Builds {
    pub button: gtk::MenuButton,
    list: gtk::Box,
}

impl Builds {
    pub(crate) fn new() -> Self {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let hint = gtk::Label::new(Some(
            "They keep running on their old build and update when you open new ones.",
        ));
        hint.add_css_class("dim-label");
        hint.set_wrap(true);
        hint.set_xalign(0.0);
        hint.set_max_width_chars(36);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(12);
        body.set_margin_end(12);
        body.set_margin_top(12);
        body.set_margin_bottom(12);
        body.append(&hint);
        body.append(&list);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&body));
        let button = gtk::MenuButton::new();
        button.set_popover(Some(&popover));
        button.set_visible(false);
        button.set_focusable(false);
        Self { button, list }
    }
}

impl App {
    fn older_terms(&self) -> Vec<TermInfo> {
        self.holders
            .list()
            .into_iter()
            .filter(|t| t.build.older)
            .collect()
    }

    pub(crate) fn build_notes(&self) -> HashMap<TermId, String> {
        let older = self.older_terms();
        if older.is_empty() {
            return HashMap::new();
        }
        let installed = xenon_term::installed_build();
        older
            .into_iter()
            .map(|t| {
                let note = format!(
                    "Running build {}. New terminals use {installed}.",
                    t.build.label
                );
                (t.id, note)
            })
            .collect()
    }

    pub(crate) fn refresh_builds(self: &Rc<Self>) {
        let older = self.older_terms();
        let builds = &self.builds;
        builds.button.set_visible(!older.is_empty());
        builds
            .button
            .set_label(&format!("{} on an older build", older.len()));
        while let Some(row) = builds.list.first_child() {
            builds.list.remove(&row);
        }
        let model = self.model.borrow();
        for term in &older {
            let tab = model.tab_of_term(term.id);
            let workspace = model
                .rec(term.workspace)
                .map_or_else(|| "Closed workspace".to_owned(), |r| r.name.clone());
            let title = if term.status.title.is_empty() {
                "terminal"
            } else {
                &term.status.title
            };
            let row = gtk::Button::new();
            row.add_css_class("flat");
            row.set_sensitive(tab.is_some());
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let name = gtk::Label::new(Some(&format!("{workspace} \u{b7} {title}")));
            name.set_xalign(0.0);
            name.set_hexpand(true);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let label = gtk::Label::new(Some(&term.build.label));
            label.add_css_class("dim-label");
            line.append(&name);
            line.append(&label);
            row.set_child(Some(&line));
            if let Some((ws, tab)) = tab {
                let weak = Rc::downgrade(self);
                row.connect_clicked(move |_| {
                    defer(&weak, move |app| {
                        app.builds.button.popdown();
                        app.show_tab(ws, tab);
                    });
                });
            }
            builds.list.append(&row);
        }
        drop(model);
        self.paint_all();
    }

    fn show_tab(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        if self.model.borrow().active() != Some(ws) {
            self.activate_workspace(ws);
        }
        self.select_tab_ui(ws, tab);
    }

    pub(crate) fn unreadable_notice(self: &Rc<Self>, listed: &TermInfo) -> gtk::Widget {
        let page = adw::StatusPage::new();
        page.set_icon_name(Some("software-update-available-symbolic"));
        page.set_title("This terminal needs a newer Xenon window");
        page.set_description(Some(&format!(
            "It runs build {}, which this window cannot read. It keeps running; closing the tab ends it.",
            listed.build.label
        )));
        let button = gtk::Button::with_label("Restart Window");
        button.add_css_class("suggested-action");
        button.add_css_class("pill");
        button.set_halign(gtk::Align::Center);
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| defer(&weak, |app| app.restart_window()));
        page.set_child(Some(&button));
        page.upcast()
    }

    fn restart_window(self: &Rc<Self>) {
        let launcher = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default()
            .join(".local/bin/xenon");
        let started = std::process::Command::new("setsid")
            .args(["-f", "sh", "-c", "sleep 1; exec \"$0\""])
            .arg(launcher)
            .spawn();
        match started {
            Ok(_) => self.quit(),
            Err(error) => self.toast(&format!("Couldn\u{2019}t restart the window: {error}")),
        }
    }
}
