use std::rc::Rc;

use gtk::{glib, prelude::*};
use std::path::Path;
use xenon_settings::MarkdownMode;

use super::EditorView;

pub(super) fn add_row(path: &Path, root: &gtk::Box) -> adw::ToggleGroup {
    let switch = view_switch(path);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("toolbar");
    row.append(&switch);
    root.append(&row);
    switch
}

fn view_switch(path: &Path) -> adw::ToggleGroup {
    let switch = adw::ToggleGroup::new();
    let mut views = vec![
        ("prose", "Markdown", "Formatted Markdown \u{b7} Alt+1"),
        (
            "source",
            "Code",
            "Markdown source with syntax colors \u{b7} Alt+3",
        ),
    ];
    if super::list::is_worklist(path) {
        views.push(("list", "Tasks", "Task list \u{b7} Alt+4"));
    }
    for (name, label, tip) in views {
        let toggle = adw::Toggle::new();
        toggle.set_name(Some(name));
        toggle.set_label(Some(label));
        toggle.set_tooltip(tip);
        switch.add(toggle);
    }
    switch
}

impl EditorView {
    pub(super) fn connect_switch(self: &Rc<Self>) {
        let Some(switch) = &self.switch else { return };
        let weak = Rc::downgrade(self);
        switch.connect_active_name_notify(move |switch| {
            let Some(this) = weak.upgrade() else { return };
            if this.syncing.get() {
                return;
            }
            let name = switch.active_name().unwrap_or_default();
            glib::idle_add_local_once(move || match name.as_str() {
                "list" => {
                    this.set_list(true);
                }
                "prose" => {
                    this.set_list(false);
                    this.set_mode(MarkdownMode::Prose);
                }
                _ => {
                    this.set_list(false);
                    this.set_mode(MarkdownMode::Source);
                }
            });
        });
    }

    pub(super) fn sync_switch(&self) {
        let Some(switch) = &self.switch else { return };
        let name = if self.list_showing() {
            "list"
        } else if self.mode.get() == MarkdownMode::Source {
            "source"
        } else {
            "prose"
        };
        self.syncing.set(true);
        switch.set_active_name(Some(name));
        self.syncing.set(false);
    }
}
