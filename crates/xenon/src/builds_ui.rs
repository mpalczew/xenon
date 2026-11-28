use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use xenon_core::TermId;
use xenon_term::TermInfo;

use crate::app::{App, defer};

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
