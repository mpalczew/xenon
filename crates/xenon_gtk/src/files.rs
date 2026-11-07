//! The Files section of the sidebar: the workspace folder as a tree. The tree
//! rules (`FilesModel`: which rows show, in what order) are plain data; the
//! view makes one stock `GtkListBox` row per visible entry. Directories are
//! read on a worker thread when first opened, and only expanded folders are
//! watched (a cap keeps the number of watchers bounded). Like the GPUI tree
//! it lists every entry, folders first.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use xenon_finder::tree::list_dir;

use crate::files_rows::{FilesModel, Row, make_row};

use crate::app::App;

/// Most folders watched at once; deeper expansions are not watched.
const MAX_WATCHED: usize = 48;
/// Coalesce a burst of file events into one reload.
const RELOAD_DELAY: Duration = Duration::from_millis(250);

pub(crate) struct FilesView {
    pub root: gtk::Box,
    pub list: gtk::ListBox,
    title: gtk::Label,
    app: Weak<App>,
    model: RefCell<Option<FilesModel>>,
    rows: RefCell<Vec<Row>>,
    monitors: RefCell<HashMap<PathBuf, gio::FileMonitor>>,
    dirty: RefCell<HashSet<PathBuf>>,
    reload_armed: Cell<bool>,
    /// File shown in the active editor tab.
    open_file: RefCell<Option<PathBuf>>,
    /// Bumped when the root changes, so a late directory read is dropped.
    epoch: Cell<u64>,
}

impl FilesView {
    pub(crate) fn new(app: Weak<App>) -> Rc<Self> {
        let title = gtk::Label::new(Some("FILES"));
        title.add_css_class("xheading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        header.add_css_class("xsection-head");
        header.append(&title);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.add_css_class("xfiles");
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&list));
        scroll.set_vexpand(true);
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("xsection");
        root.append(&header);
        root.append(&scroll);
        let this = Rc::new(Self {
            root,
            list,
            title,
            app,
            model: RefCell::new(None),
            rows: RefCell::default(),
            monitors: RefCell::default(),
            dirty: RefCell::default(),
            reload_armed: Cell::new(false),
            open_file: RefCell::new(None),
            epoch: Cell::new(0),
        });
        this.wire();
        this
    }

    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.list.connect_row_activated(move |_, row| {
            let Some(this) = weak.upgrade() else { return };
            let picked = this.rows.borrow().get(row.index().max(0) as usize).cloned();
            if let Some(picked) = picked {
                this.activate(&picked);
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            let handled = match key {
                gdk::Key::Right | gdk::Key::l => this.step_in(),
                gdk::Key::Left | gdk::Key::h => this.step_out(),
                gdk::Key::Escape => {
                    if let Some(app) = this.app.upgrade() {
                        app.focus_terminal();
                    }
                    true
                }
                _ => false,
            };
            if handled {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.list.add_controller(keys);
        // Scripts read `files-state`: keep it current when focus or selection moves.
        let focus = gtk::EventControllerFocus::new();
        let publish = |weak: Weak<Self>| {
            move |_: &gtk::EventControllerFocus| {
                if let Some(app) = weak.upgrade().and_then(|t| t.app.upgrade()) {
                    app.publish_files();
                }
            }
        };
        focus.connect_enter(publish(Rc::downgrade(self)));
        focus.connect_leave(publish(Rc::downgrade(self)));
        self.list.add_controller(focus);
        let weak = Rc::downgrade(self);
        self.list.connect_row_selected(move |_, _| {
            if let Some(app) = weak.upgrade().and_then(|t| t.app.upgrade()) {
                app.publish_files();
            }
        });
    }

    fn selected_row(&self) -> Option<Row> {
        let row = self.list.selected_row()?;
        self.rows.borrow().get(row.index().max(0) as usize).cloned()
    }

    fn activate(self: &Rc<Self>, row: &Row) {
        if row.is_dir {
            self.toggle(&row.path);
        } else if let Some(app) = self.app.upgrade() {
            let ws = app.model.borrow().active();
            if let Some(ws) = ws {
                app.open_editor(ws, &row.path);
            }
        }
    }

    /// Right: open a closed folder, or step into an open one.
    fn step_in(self: &Rc<Self>) -> bool {
        let Some(row) = self.selected_row().filter(|r| r.is_dir) else {
            return false;
        };
        if !row.expanded {
            self.toggle(&row.path);
        } else if let Some(next) = self
            .list
            .row_at_index(self.list.selected_row().map_or(0, |r| r.index()) + 1)
        {
            self.list.select_row(Some(&next));
            next.grab_focus();
        }
        true
    }

    /// Left: close an open folder, or step out to the parent.
    fn step_out(self: &Rc<Self>) -> bool {
        let Some(row) = self.selected_row() else {
            return false;
        };
        if row.is_dir && row.expanded {
            self.toggle(&row.path);
        } else if let Some(parent) = row.path.parent() {
            self.select_path(parent, true);
        }
        true
    }

    /// Show `root` (the workspace folder), or nothing.
    pub(crate) fn set_root(self: &Rc<Self>, root: Option<PathBuf>) {
        let same = self.model.borrow().as_ref().map(|m| &m.root) == root.as_ref();
        if same {
            return;
        }
        self.epoch.set(self.epoch.get() + 1);
        self.monitors.borrow_mut().clear();
        self.rows.borrow_mut().clear();
        *self.model.borrow_mut() = root.clone().map(|root| FilesModel {
            root,
            expanded: HashSet::new(),
            dirs: HashMap::new(),
        });
        self.root.set_visible(root.is_some());
        if let Some(root) = root {
            self.title.set_text("FILES");
            self.load(&root);
        }
        self.rebuild();
    }

    /// Read `dir` on a worker thread and show it when it arrives.
    fn load(self: &Rc<Self>, dir: &Path) {
        let epoch = self.epoch.get();
        let dir = dir.to_path_buf();
        self.watch(&dir);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let path = dir.clone();
            let Ok(entries) = gio::spawn_blocking(move || list_dir(&path)).await else {
                return;
            };
            let Some(this) = weak.upgrade() else { return };
            if this.epoch.get() != epoch {
                return;
            }
            if let Some(model) = this.model.borrow_mut().as_mut() {
                model.dirs.insert(dir, entries);
            }
            this.rebuild();
        });
    }

    fn watch(self: &Rc<Self>, dir: &Path) {
        if self.monitors.borrow().len() >= MAX_WATCHED || self.monitors.borrow().contains_key(dir) {
            return;
        }
        let file = gio::File::for_path(dir);
        let Ok(monitor) =
            file.monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)
        else {
            return;
        };
        let (weak, path) = (Rc::downgrade(self), dir.to_path_buf());
        monitor.connect_changed(move |_, _, _, _| {
            let Some(this) = weak.upgrade() else { return };
            this.dirty.borrow_mut().insert(path.clone());
            if this.reload_armed.replace(true) {
                return;
            }
            let weak = Rc::downgrade(&this);
            glib::timeout_add_local_once(RELOAD_DELAY, move || {
                let Some(this) = weak.upgrade() else { return };
                this.reload_armed.set(false);
                let dirty: Vec<PathBuf> = this.dirty.borrow_mut().drain().collect();
                for dir in dirty {
                    this.load(&dir);
                }
            });
        });
        self.monitors
            .borrow_mut()
            .insert(dir.to_path_buf(), monitor);
    }

    pub(crate) fn toggle(self: &Rc<Self>, dir: &Path) {
        let opened = {
            let mut model = self.model.borrow_mut();
            let Some(model) = model.as_mut() else { return };
            if model.expanded.remove(dir) {
                false
            } else {
                model.expanded.insert(dir.to_path_buf());
                true
            }
        };
        if opened {
            self.load(dir);
        } else {
            // Closed folders are no longer watched.
            self.monitors
                .borrow_mut()
                .retain(|p, _| !p.starts_with(dir));
            self.rebuild();
        }
        self.select_path(dir, true);
    }

    /// Open every folder on the way to `dir`, show it, and focus the tree.
    pub(crate) fn reveal(self: &Rc<Self>, dir: &Path) {
        let mut missing = Vec::new();
        {
            let mut model = self.model.borrow_mut();
            let Some(model) = model.as_mut() else { return };
            if !dir.starts_with(&model.root) {
                return;
            }
            let mut at = Some(dir);
            while let Some(path) = at {
                if path == model.root {
                    break;
                }
                if model.expanded.insert(path.to_path_buf()) {
                    missing.push(path.to_path_buf());
                }
                at = path.parent();
            }
        }
        for path in missing {
            self.load(&path);
        }
        self.rebuild();
        self.select_path(dir, true);
    }

    /// Mark the file shown in the active editor tab.
    pub(crate) fn mark_open(&self, path: Option<PathBuf>) {
        if *self.open_file.borrow() == path {
            return;
        }
        *self.open_file.borrow_mut() = path;
        self.update_open_marks();
    }

    fn update_open_marks(&self) {
        let open = self.open_file.borrow().clone();
        let rows = self.rows.borrow();
        for (i, row) in rows.iter().enumerate() {
            if let Some(widget) = self.list.row_at_index(i as i32) {
                crate::pane::set_class(
                    &widget,
                    "xopen",
                    open.as_deref() == Some(row.path.as_path()),
                );
            }
        }
    }

    fn select_path(&self, path: &Path, focus: bool) {
        let index = self.rows.borrow().iter().position(|r| r.path == path);
        if let Some(row) = index.and_then(|i| self.list.row_at_index(i as i32)) {
            self.list.select_row(Some(&row));
            if focus {
                row.grab_focus();
            }
        }
    }

    /// Put the keyboard in the tree.
    pub(crate) fn focus(&self) {
        let target = self
            .list
            .selected_row()
            .or_else(|| self.list.row_at_index(0));
        if let Some(row) = target {
            self.list.select_row(Some(&row));
            row.grab_focus();
        }
        self.publish();
    }

    fn publish(&self) {
        if let Some(app) = self.app.upgrade() {
            app.publish_files();
        }
    }

    /// JSON picture of the tree for scripts (`app.files-state`).
    pub(crate) fn snapshot(&self) -> String {
        let rows: Vec<serde_json::Value> = self
            .rows
            .borrow()
            .iter()
            .take(80)
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "depth": r.depth,
                    "dir": r.is_dir,
                    "expanded": r.expanded,
                    "open": self.open_file.borrow().as_deref() == Some(r.path.as_path()),
                })
            })
            .collect();
        serde_json::json!({
            "rows": rows,
            "selected": self.selected_row().map(|r| r.name),
            "focused": crate::pane::focus_inside(&self.list),
        })
        .to_string()
    }

    /// A key by name (`up`, `down`, `left`, `right`, `enter`), for scripts.
    pub(crate) fn key(self: &Rc<Self>, name: &str) {
        let at = self.list.selected_row().map_or(-1, |r| r.index());
        match name {
            "down" | "up" => {
                let next = if name == "down" {
                    at + 1
                } else {
                    (at - 1).max(0)
                };
                if let Some(row) = self.list.row_at_index(next) {
                    self.list.select_row(Some(&row));
                    row.grab_focus();
                }
            }
            "right" => {
                self.step_in();
            }
            "left" => {
                self.step_out();
            }
            "enter" => {
                if let Some(row) = self.selected_row() {
                    self.activate(&row);
                }
            }
            _ => {}
        }
        self.publish();
    }

    pub(crate) fn has_rows(&self) -> bool {
        !self.rows.borrow().is_empty()
    }

    /// Redraw the rows from the model, keeping selection and focus.
    fn rebuild(&self) {
        let keep = self.selected_row().map(|r| r.path);
        let had_focus = crate::pane::focus_inside(&self.list);
        let rows = self
            .model
            .borrow()
            .as_ref()
            .map(FilesModel::rows)
            .unwrap_or_default();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        for row in &rows {
            self.list.append(&make_row(row));
        }
        *self.rows.borrow_mut() = rows;
        self.update_open_marks();
        if let Some(path) = keep {
            self.select_path(&path, had_focus);
        }
        if let Some(app) = self.app.upgrade() {
            app.publish_files();
        }
    }
}
