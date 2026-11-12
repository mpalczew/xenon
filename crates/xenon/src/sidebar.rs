use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use xenon_core::{WorkspaceDot, WorkspaceId};
use xenon_git::status::DirtState;

use crate::app::{App, defer};
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
    rows_box: gtk::Overlay,
    outer: gtk::Paned,
    inner: gtk::Paned,
    pub list: gtk::ListBox,
    add: gtk::MenuButton,
    app: Weak<App>,
    rows: RefCell<Vec<WsRow>>,
    dragging: Rc<Cell<Option<usize>>>,
}

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
        list.connect_row_activated(move |_, row| {
            let index = row.index().max(0) as usize;
            defer(&weak, move |app| {
                app.activate_workspace_index(index);
            });
        });
        let keys = gtk::EventControllerKey::new();
        let weak = app.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(app) = weak.upgrade()
            {
                glib::idle_add_local_once(move || app.focus_terminal());
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        list.add_controller(keys);
        list.add_css_class("xworkspaces");
        let rule = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        rule.set_valign(gtk::Align::Start);
        rule.set_can_target(false);
        rule.set_visible(false);
        rule.set_margin_start(6);
        rule.set_margin_end(6);
        for cap in [true, false] {
            if !cap {
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                line.add_css_class("xdrop-line");
                line.set_hexpand(true);
                line.set_valign(gtk::Align::Center);
                rule.append(&line);
            }
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("xdrop-cap");
            rule.append(&dot);
        }
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&list));
        overlay.add_overlay(&rule);
        let dragging = Rc::new(Cell::new(None));
        Self::accept_drops(&list, &rule, &dragging, app.clone());

        let workspaces = Section::new("WORKSPACES", Fold::Workspaces, &overlay, Some(300), &app);
        workspaces.set_trailing(&add);
        workspaces.sep.set_visible(false);
        let files = FilesView::new(app.clone());
        let git = GitView::new(app.clone());
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("sidebar-pane");
        let inner = gtk::Paned::new(gtk::Orientation::Vertical);
        inner.set_shrink_start_child(false);
        inner.set_shrink_end_child(false);
        inner.set_start_child(Some(&files.root));
        inner.set_end_child(Some(&git.root));
        let outer = gtk::Paned::new(gtk::Orientation::Vertical);
        outer.set_vexpand(true);
        outer.set_shrink_start_child(false);
        outer.set_shrink_end_child(false);
        outer.set_resize_start_child(false);
        outer.set_start_child(Some(&workspaces.root));
        outer.set_end_child(Some(&inner));
        root.append(&outer);
        Self {
            root,
            outer,
            inner,
            files,
            git,
            workspaces,
            rows_box: overlay,
            list,
            add,
            app,
            rows: RefCell::default(),
            dragging,
        }
    }

    fn accept_drops(
        list: &gtk::ListBox,
        rule: &gtk::Box,
        dragging: &Rc<Cell<Option<usize>>>,
        app: Weak<App>,
    ) {
        let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        let slot_at = |list: &gtk::ListBox, y: f64| {
            let rows = list.observe_children().n_items() as i32;
            let bounds = |i: i32| {
                let b = list.row_at_index(i)?.compute_bounds(list)?;
                Some((b.y() as f64, (b.y() + b.height()) as f64))
            };
            let slot = (0..rows)
                .find(|&i| bounds(i).is_some_and(|(top, bottom)| y < (top + bottom) / 2.0))
                .unwrap_or(rows);
            let line = match slot {
                0 => bounds(0).map(|b| b.0),
                s if s == rows => bounds(rows - 1).map(|b| b.1),
                s => bounds(s - 1).zip(bounds(s)).map(|(a, b)| (a.1 + b.0) / 2.0),
            };
            (slot as usize, line.unwrap_or(0.0))
        };
        let moves = {
            let dragging = dragging.clone();
            move |slot: usize| dragging.get().is_some_and(|d| slot != d && slot != d + 1)
        };
        target.connect_motion({
            let list = list.clone();
            let rule = rule.clone();
            let moves = moves.clone();
            move |_, _, y| {
                let (slot, line) = slot_at(&list, y);
                rule.set_margin_top(line as i32 - 3);
                rule.set_visible(moves(slot));
                gdk::DragAction::MOVE
            }
        });
        target.connect_leave({
            let rule = rule.clone();
            move |_| rule.set_visible(false)
        });
        target.connect_drop({
            let list = list.clone();
            let rule = rule.clone();
            move |_, value, _, y| {
                rule.set_visible(false);
                let (slot, _) = slot_at(&list, y);
                let Ok(from) = value.get::<u32>() else {
                    return false;
                };
                if !moves(slot) {
                    return false;
                }
                let end = slot == list.observe_children().n_items() as usize;
                defer(&app, move |app| {
                    app.reorder_workspace(from as usize, (!end).then_some(slot));
                });
                true
            }
        });
        list.add_controller(target);
    }

    pub(crate) fn set_hints(&self, app: &App) {
        self.add
            .set_tooltip_text(Some(&app.tip("Open Workspace", "app.open-workspace")));
        self.git.set_hint(&app.tip("Git", "app.focus-git"));
    }

    pub(crate) fn apply_folds(&self, model: &Model) {
        let folds = model.folds;
        self.workspaces.show_fold(folds.workspaces);
        self.files.show_fold(folds.files);
        self.git.show_fold(folds.git);
        self.inner.set_resize_start_child(!folds.files);
        self.inner.set_resize_end_child(folds.files);
    }

    pub(crate) fn restore_sizes(&self, workspaces: Option<f32>, files: Option<f32>) {
        for (paned, size) in [(&self.outer, workspaces), (&self.inner, files)] {
            if let Some(size) = size {
                paned.set_position(size as i32);
            }
            let app = self.app.clone();
            paned.connect_position_notify(move |_| {
                defer(&app, move |app| {
                    app.schedule_save();
                });
            });
        }
    }

    /// Opens the workspaces section tall enough for every row; the divider stops where
    /// files and git still keep their minimum height.
    fn fit_workspaces(&self) {
        let width = self.root.width();
        let height = |w: &gtk::Widget| w.measure(gtk::Orientation::Vertical, width).1;
        let rows = height(self.rows_box.upcast_ref());
        let head = height(self.workspaces.head().upcast_ref());
        self.outer.set_position(head + rows);
    }

    pub(crate) fn sizes(&self) -> (Option<f32>, Option<f32>) {
        let size = |p: &gtk::Paned| p.is_position_set().then(|| p.position() as f32);
        (size(&self.outer), size(&self.inner))
    }

    pub(crate) fn reset_sizes(&self) {
        self.inner.set_property("position-set", false);
        self.outer.set_property("position-set", false);
    }

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
                defer(&weak, move |app| {
                    app.close_workspace_id(id);
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
                let dragging = self.dragging.clone();
                let name = rec.name.clone();
                move |_, drag| {
                    dragging.set(Some(row.index().max(0) as usize));
                    row.add_css_class("xdragging");
                    let ghost = gtk::Label::new(Some(&name));
                    ghost.add_css_class("xdrag-ghost");
                    ghost.set_margin_start(14);
                    ghost.set_margin_top(20);
                    if let Some(icon) =
                        gtk::DragIcon::for_drag(drag).downcast_ref::<gtk::DragIcon>()
                    {
                        // sync: answer or ordering before return: the icon window is new and belongs to this drag
                        icon.set_child(Some(&ghost));
                    }
                }
            });
            drag.connect_drag_end({
                let row = row.clone();
                let dragging = self.dragging.clone();
                move |_, _, _| {
                    dragging.set(None);
                    row.remove_css_class("xdragging");
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
    pub(crate) fn set_fold(&self, fold: Fold, folded: bool) {
        {
            let mut model = self.model.borrow_mut();
            if model.folds.get(fold) == folded {
                return;
            }
            model.folds.set(fold, folded);
            model.folds.save();
        }
        self.sidebar.reset_sizes();
        self.sidebar.apply_folds(&self.model.borrow());
        if matches!(fold, Fold::Workspaces) && !folded {
            self.sidebar.fit_workspaces();
        }
        if let Some(app) = self.sidebar.app.upgrade() {
            app.schedule_save();
        }
    }

    pub(crate) fn toggle_fold(&self, fold: Fold) {
        let folded = self.model.borrow().folds.get(fold);
        self.set_fold(fold, !folded);
    }
}
