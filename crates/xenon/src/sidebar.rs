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
use crate::model::{Fold, Model};
use crate::pane::set_class;
use crate::sidebar_ui::{Row, Section};

struct WsRow {
    id: WorkspaceId,
    row: gtk::ListBoxRow,
    title: gtk::Label,
    dot: gtk::Image,
    tick: gtk::Image,
}

pub(crate) struct Sidebar {
    pub root: gtk::Box,
    pub files: Rc<FilesView>,
    pub git: Rc<GitView>,
    workspaces: Section,
    pub list: gtk::ListBox,
    add: gtk::MenuButton,
    app: Weak<App>,
    rows: RefCell<Vec<WsRow>>,
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
        let menu = gio::Menu::new();
        menu.append(Some("New workspace\u{2026}"), Some("app.new-workspace"));
        menu.append(Some("Open workspace\u{2026}"), Some("app.open-workspace"));
        let add = gtk::MenuButton::new();
        add.set_icon_name("list-add-symbolic");
        add.set_menu_model(Some(&menu));
        add.add_css_class("flat");
        add.set_focusable(false);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Browse);
        list.set_activate_on_single_click(true);
        list.add_css_class("navigation-sidebar");
        let weak = app.clone();
        // Deferred: rebuilding the list inside its own activate handler crashes GTK.
        list.connect_row_activated(move |_, row| {
            let index = row.index().max(0) as usize;
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(app) = weak.upgrade() {
                    app.activate_workspace_index(index);
                }
            });
        });
        let keys = gtk::EventControllerKey::new();
        let weak = app.clone();
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
        Self::accept_drops(&list, app.clone());

        let workspaces = Section::new("WORKSPACES", Fold::Workspaces, &list, Some(300), &app);
        workspaces.set_trailing(&add);
        workspaces.sep.set_visible(false);
        let files = FilesView::new(app.clone());
        let git = GitView::new(app.clone());
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("sidebar-pane");
        root.append(&workspaces.root);
        root.append(&files.root);
        root.append(&git.root);
        Self {
            root,
            files,
            git,
            workspaces,
            list,
            add,
            app,
            rows: RefCell::default(),
        }
    }

    /// Dropping a dragged row on the list: a row takes the dragged one's place,
    /// the space below the last row sends it to the end.
    fn accept_drops(list: &gtk::ListBox, app: Weak<App>) {
        let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        // Mark where the row would land; `None` clears every mark.
        let mark = |list: &gtk::ListBox, y: Option<f64>| {
            let rows = list.observe_children().n_items() as i32;
            let hit = y.and_then(|y| list.row_at_y(y as i32));
            for i in 0..rows {
                if let Some(row) = list.row_at_index(i) {
                    set_class(
                        &row,
                        "drop-before",
                        y.is_some() && hit.as_ref() == Some(&row),
                    );
                    set_class(
                        &row,
                        "drop-after",
                        y.is_some() && hit.is_none() && i == rows - 1,
                    );
                }
            }
            hit
        };
        target.connect_motion({
            let list = list.clone();
            move |_, _, y| {
                mark(&list, Some(y));
                gdk::DragAction::MOVE
            }
        });
        target.connect_leave({
            let list = list.clone();
            move |_| {
                mark(&list, None);
            }
        });
        target.connect_drop({
            let list = list.clone();
            move |_, value, _, y| {
                let hit = list.row_at_y(y as i32).map(|r| r.index().max(0) as usize);
                mark(&list, None);
                let Ok(from) = value.get::<u32>() else {
                    return false;
                };
                // Deferred: reordering rebuilds the list mid-drop.
                let app = app.clone();
                glib::idle_add_local_once(move || {
                    if let Some(app) = app.upgrade() {
                        app.reorder_workspace(from as usize, hit);
                    }
                });
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

    /// Draw each section folded or open as the model says.
    pub(crate) fn apply_folds(&self, model: &Model) {
        let folds = model.folds;
        self.workspaces.show_fold(folds.workspaces);
        self.files.show_fold(folds.files);
        self.git.show_fold(folds.git);
        // Open Files takes the free space; Workspaces and Git size to their
        // content up to a cap. With Files folded the free space stays below.
        self.files.root.set_vexpand(!folds.files);
    }

    /// Rebuild the rows from the registry and select the active one.
    pub(crate) fn rebuild(&self, model: &Model) {
        self.apply_folds(model);
        let mut rows = self.rows.borrow_mut();
        self.list.remove_all();
        rows.clear();
        for rec in &model.registry.workspaces {
            let line = Row::new();
            line.set(None, &rec.name, &short_path(&rec.display_root()));
            line.stack();
            line.root.set_tooltip_text(Some(&rec.display_root()));
            let dot = gtk::Image::from_icon_name("media-record-symbolic");
            let tick = gtk::Image::from_icon_name("document-edit-symbolic");
            let status = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            status.append(&dot);
            status.append(&tick);
            let close = gtk::Button::from_icon_name("window-close-symbolic");
            close.set_has_frame(false);
            close.set_focusable(false);
            if let Some(app) = self.app.upgrade() {
                close.set_tooltip_text(Some(&app.tip("Close", "app.close-workspace")));
            }
            let weak = self.app.clone();
            let id = rec.id;
            close.connect_clicked(move |_| {
                let weak = weak.clone();
                // Deferred: closing rebuilds the list that holds this button.
                glib::idle_add_local_once(move || {
                    if let Some(app) = weak.upgrade() {
                        app.close_workspace_id(id);
                    }
                });
            });
            line.swap_on_hover(&status, &close);
            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&line.root));
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
            rows.push(WsRow {
                id: rec.id,
                row,
                title: line.title.clone(),
                dot,
                tick,
            });
        }
    }

    /// Select the active row and mark it; paint each row's status.
    pub(crate) fn paint(&self, model: &Model, git: &GitHub) {
        let active = model.active();
        for r in self.rows.borrow().iter() {
            if Some(r.id) == active {
                self.list.select_row(Some(&r.row));
            }
            set_class(&r.title, "heading", Some(r.id) == active);
            let dot = model.workspace_dot(r.id);
            let working = matches!(dot, Some(WorkspaceDot::Working));
            let attention = matches!(dot, Some(WorkspaceDot::Attention(_)));
            r.dot.set_visible(working || attention);
            set_class(&r.dot, "accent", working);
            set_class(&r.dot, "warning", attention);
            r.dot.set_opacity(1.0);
            let dirt = git.dirt(r.id);
            r.tick.set_visible(dirt != DirtState::Clean);
            set_class(&r.tick, "warning", dirt == DirtState::Dirty);
            set_class(&r.tick, "dim-label", dirt == DirtState::IndexOnly);
            r.tick.set_tooltip_text(match dirt {
                DirtState::Clean => None,
                DirtState::Dirty => Some("Changes since HEAD"),
                DirtState::IndexOnly => Some("Staged changes only"),
            });
        }
    }

    pub(crate) fn pulse(&self, opacity: f64) {
        for r in self.rows.borrow().iter() {
            if r.dot.has_css_class("accent") {
                r.dot.set_opacity(opacity);
            }
        }
    }
}

impl App {
    /// Fold or unfold a sidebar section and remember it.
    pub(crate) fn set_fold(&self, fold: Fold, folded: bool) {
        {
            let mut model = self.model.borrow_mut();
            if model.folds.get(fold) == folded {
                return;
            }
            model.folds.set(fold, folded);
            model.folds.save();
        }
        self.sidebar.apply_folds(&self.model.borrow());
    }

    pub(crate) fn toggle_fold(&self, fold: Fold) {
        let folded = self.model.borrow().folds.get(fold);
        self.set_fold(fold, !folded);
    }
}
