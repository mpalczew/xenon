//! The workspaces sidebar: a list box with a compact two-line row per
//! workspace and a status line in the left margin.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use xenon_core::{WorkspaceDot, WorkspaceId};
use xenon_git::status::DirtState;

use crate::app::App;
use crate::files::FilesView;
use crate::git::GitHub;
use crate::git_section::GitView;
use crate::model::Model;
use crate::pane::set_class;

struct Row {
    id: WorkspaceId,
    row: gtk::ListBoxRow,
    body: gtk::Box,
    dot: gtk::Box,
    tick: gtk::Box,
}

pub(crate) struct Sidebar {
    pub root: gtk::Box,
    pub files: Rc<FilesView>,
    pub git: Rc<GitView>,
    split: gtk::Paned,
    pub list: gtk::ListBox,
    add: gtk::MenuButton,
    app: Weak<App>,
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
        header.add_css_class("xsection-head");
        header.add_css_class("xfirst");
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
        let app_for_files = app.clone();
        let app_for_git = app.clone();
        let app_for_rows = app.clone();
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
        Self::accept_drops(&list, app_for_rows.clone());
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(340);

        let workspaces = gtk::Box::new(gtk::Orientation::Vertical, 0);
        workspaces.add_css_class("xsection");
        workspaces.append(&header);
        workspaces.append(&scroll);
        let files = FilesView::new(app_for_files);
        let split = gtk::Paned::new(gtk::Orientation::Vertical);
        split.set_start_child(Some(&workspaces));
        split.set_end_child(Some(&files.root));
        split.set_shrink_start_child(false);
        split.set_shrink_end_child(false);
        split.set_resize_start_child(false);
        split.set_wide_handle(false);
        let git = GitView::new(app_for_git);
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("xsidebar");
        root.append(&split);
        root.append(&git.root);
        split.set_vexpand(true);
        Self {
            root,
            files,
            git,
            split,
            list,
            add,
            app: app_for_rows,
            rows: RefCell::default(),
        }
    }

    /// Dropping a dragged row on the list: a row takes the dragged one's place,
    /// the space below the last row sends it to the end.
    fn accept_drops(list: &gtk::ListBox, app: Weak<App>) {
        let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        let mark = |list: &gtk::ListBox, y: f64| {
            let rows = list.observe_children().n_items() as i32;
            let hit = list.row_at_y(y as i32);
            for i in 0..rows {
                if let Some(row) = list.row_at_index(i) {
                    set_class(&row, "drop-before", hit.as_ref() == Some(&row));
                    set_class(&row, "drop-after", hit.is_none() && i == rows - 1);
                }
            }
            hit
        };
        target.connect_motion({
            let list = list.clone();
            move |_, _, y| {
                mark(&list, y);
                gdk::DragAction::MOVE
            }
        });
        target.connect_leave({
            let list = list.clone();
            move |_| {
                mark(&list, -1.0);
            }
        });
        target.connect_drop({
            let list = list.clone();
            move |_, value, _, y| {
                let hit = mark(&list, -1.0).or_else(|| list.row_at_y(y as i32));
                let (Ok(from), Some(app)) = (value.get::<u32>(), app.upgrade()) else {
                    return false;
                };
                app.reorder_workspace(from as usize, hit.map(|r| r.index().max(0) as usize));
                true
            }
        });
        list.add_controller(target);
    }

    /// Tooltips that show keys; they need the key map, which exists once the
    /// actions are installed.
    pub(crate) fn set_hints(&self, app: &App) {
        self.add
            .set_tooltip_text(Some(&app.tip("Open Workspace", "app.open-workspace")));
        self.git.set_hint(&app.tip("Git", "app.focus-git"));
    }

    /// Rebuild the rows from the registry and select the active one.
    pub(crate) fn rebuild(&self, model: &Model) {
        // Room for every workspace row (up to a cap), the Files section gets the rest.
        let wanted = 72 + 47 * model.registry.workspaces.len() as i32;
        self.split.set_position(wanted.clamp(110, 380));
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
            path.set_ellipsize(gtk::pango::EllipsizeMode::Start);
            let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
            text.append(&name);
            text.append(&path);
            let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            body.add_css_class("xrow");
            let glyph = gtk::Image::from_icon_name("folder-symbolic");
            glyph.add_css_class("xrow-icon");
            glyph.set_valign(gtk::Align::Start);
            body.append(&glyph);
            body.append(&text);
            body.set_tooltip_text(Some(&rec.display_root()));
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("xdot");
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            line.append(&dot);
            line.append(&body);
            body.set_hexpand(true);
            let tick = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            tick.add_css_class("xtick");
            tick.set_valign(gtk::Align::Center);
            tick.set_halign(gtk::Align::End);
            let close = gtk::Button::from_icon_name("window-close-symbolic");
            close.add_css_class("flat");
            close.add_css_class("xrow-close");
            close.set_focusable(false);
            close.set_valign(gtk::Align::Center);
            if let Some(app) = self.app.upgrade() {
                close.set_tooltip_text(Some(&app.tip("Close", "app.close-workspace")));
            }
            let weak = self.app.clone();
            let id = rec.id;
            close.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.close_workspace_id(id);
                }
            });
            let gutter = gtk::Overlay::new();
            gutter.add_css_class("xgutter");
            gutter.set_child(Some(&tick));
            gutter.add_overlay(&close);
            line.append(&gutter);
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&line));
            let drag = gtk::DragSource::new();
            drag.set_actions(gdk::DragAction::MOVE);
            drag.connect_prepare({
                let row = row.clone();
                move |_, _, _| {
                    Some(gdk::ContentProvider::for_value(
                        &(row.index().max(0) as u32).to_value(),
                    ))
                }
            });
            drag.connect_drag_begin({
                let row = row.clone();
                move |source, _| {
                    source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&row))), 0, 0);
                }
            });
            row.add_controller(drag);
            self.list.append(&row);
            rows.push(Row {
                id: rec.id,
                row,
                body,
                dot,
                tick,
            });
        }
    }

    /// Select the active row and mark it; paint each row's status.
    pub(crate) fn paint(&self, model: &Model, git: &GitHub) {
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
            let dirt = git.dirt(r.id);
            set_class(&r.tick, "dirty", dirt == DirtState::Dirty);
            set_class(&r.tick, "index", dirt == DirtState::IndexOnly);
            r.tick.set_tooltip_text(match dirt {
                DirtState::Clean => None,
                DirtState::Dirty => Some("Changes since HEAD"),
                DirtState::IndexOnly => Some("Staged changes only"),
            });
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
