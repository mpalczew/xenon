//! The workspaces sidebar: a list box with a compact two-line row per
//! workspace and a status line in the left margin.

use std::cell::RefCell;
use std::rc::Weak;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use xenon_core::{WorkspaceDot, WorkspaceId};

use crate::app::App;
use crate::model::Model;
use crate::pane::set_class;

struct Row {
    id: WorkspaceId,
    row: gtk::ListBoxRow,
    body: gtk::Box,
    dot: gtk::Box,
}

pub(crate) struct Sidebar {
    pub root: gtk::Box,
    pub list: gtk::ListBox,
    rows: RefCell<Vec<Row>>,
}

/// `$HOME` shown as `~`.
pub(crate) fn short_path(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}

impl Sidebar {
    pub(crate) fn new(app: Weak<App>) -> Self {
        let title = gtk::Label::new(Some("WORKSPACES"));
        title.add_css_class("xheading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let menu = gio::Menu::new();
        menu.append(Some("New workspace"), Some("app.new-workspace"));
        menu.append(Some("Open workspace"), Some("app.open-workspace"));
        let add = gtk::MenuButton::new();
        add.set_icon_name("list-add-symbolic");
        add.set_menu_model(Some(&menu));
        add.add_css_class("flat");
        add.set_focusable(false);
        add.set_tooltip_text(Some("New or open workspace"));
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.set_margin_start(14);
        header.set_margin_end(4);
        header.set_margin_top(6);
        header.append(&title);
        header.append(&add);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Browse);
        list.set_activate_on_single_click(true);
        list.add_css_class("navigation-sidebar");
        list.add_css_class("xsidebar");
        let weak = app.clone();
        list.connect_row_activated(move |_, row| {
            if let Some(app) = weak.upgrade() {
                app.activate_workspace_index(row.index().max(0) as usize);
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = app;
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(app) = weak.upgrade()
            {
                app.focus_terminal();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        list.add_controller(keys);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_vexpand(true);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
        root.add_css_class("xsidebar");
        root.append(&header);
        root.append(&scroll);
        Self {
            root,
            list,
            rows: RefCell::default(),
        }
    }

    /// Rebuild the rows from the registry and select the active one.
    pub(crate) fn rebuild(&self, model: &Model) {
        let mut rows = self.rows.borrow_mut();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        rows.clear();
        for rec in &model.registry.workspaces {
            let name = gtk::Label::new(Some(&rec.name));
            name.add_css_class("xname");
            name.set_xalign(0.0);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let path = gtk::Label::new(Some(&short_path(&rec.display_root())));
            path.add_css_class("xpath");
            path.set_xalign(0.0);
            path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
            text.append(&name);
            text.append(&path);
            let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            body.add_css_class("xrow");
            body.append(&text);
            body.set_tooltip_text(Some(&rec.display_root()));
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("xdot");
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            line.append(&dot);
            line.append(&body);
            body.set_hexpand(true);
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&line));
            self.list.append(&row);
            rows.push(Row {
                id: rec.id,
                row,
                body,
                dot,
            });
        }
    }

    /// Select the active row and mark it; paint each row's status.
    pub(crate) fn paint(&self, model: &Model) {
        let active = model.active();
        for r in self.rows.borrow().iter() {
            let selected = Some(r.id) == active;
            set_class(&r.body, "selected", selected);
            if selected {
                self.list.select_row(Some(&r.row));
            }
            let dot = model.workspace_dot(r.id);
            set_class(
                &r.dot,
                "working",
                matches!(dot, Some(WorkspaceDot::Working)),
            );
            set_class(
                &r.dot,
                "attention",
                matches!(dot, Some(WorkspaceDot::Attention(_))),
            );
            r.dot.set_opacity(1.0);
        }
    }

    pub(crate) fn pulse(&self, opacity: f64) {
        for r in self.rows.borrow().iter() {
            if r.dot.has_css_class("working") {
                r.dot.set_opacity(opacity);
            }
        }
    }
}
