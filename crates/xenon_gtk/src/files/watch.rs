//! Files section, part two: reading and watching folders, reveal, and the
//! script views of the tree.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use xenon_finder::tree::{TreeEntry, list_dir};

use super::{FilesView, MAX_WATCHED, RELOAD_DELAY, draw_item, info_for, row_path, tree_row};

impl FilesView {
    /// Read `dir` on a worker thread and show it when it arrives.
    pub(super) fn load(self: &Rc<Self>, dir: &Path) {
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
            if this.epoch.get() == epoch {
                this.apply(&dir, &entries);
            }
        });
    }

    /// Put the entries read from `dir` into its store, keeping what was open.
    pub(super) fn apply(self: &Rc<Self>, dir: &Path, entries: &[TreeEntry]) {
        let store = if self.root_dir.borrow().as_deref() == Some(dir) {
            Some(self.root_store.clone())
        } else {
            self.stores.borrow().get(dir).cloned()
        };
        let Some(store) = store else { return };
        let keep = self.selected_path();
        let had_focus = crate::pane::focus_inside(&self.list);
        let open = self.expanded_paths();
        let items: Vec<gio::FileInfo> = entries.iter().map(info_for).collect();
        store.splice(0, store.n_items(), &items);
        self.restoring.set(true);
        let mut i = 0;
        while i < self.tree.n_items() {
            if let Some(row) = tree_row(self.tree.item(i))
                && row.is_expandable()
                && !row.is_expanded()
                && row_path(&row).is_some_and(|p| open.contains(&p))
            {
                row.set_expanded(true);
            }
            i += 1;
        }
        self.restoring.set(false);
        if let Some(path) = keep {
            self.select_path(&path, had_focus);
        }
        self.advance_reveal();
        self.publish();
    }

    pub(super) fn expanded_paths(&self) -> HashSet<PathBuf> {
        (0..self.tree.n_items())
            .filter_map(|i| tree_row(self.tree.item(i)))
            .filter(gtk::TreeListRow::is_expanded)
            .filter_map(|r| row_path(&r))
            .collect()
    }

    pub(super) fn expanded_changed(self: &Rc<Self>, path: &Path, expanded: bool) {
        if expanded {
            if !self.restoring.get() {
                self.load(path);
            }
        } else {
            // Closed folders are no longer watched.
            self.monitors
                .borrow_mut()
                .retain(|p, _| !p.starts_with(path));
        }
        self.publish();
    }

    pub(super) fn watch(self: &Rc<Self>, dir: &Path) {
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

    /// Open every folder on the way to `dir`, show it, and focus the tree.
    pub(crate) fn reveal(self: &Rc<Self>, dir: &Path) {
        if !self
            .root_dir
            .borrow()
            .as_deref()
            .is_some_and(|root| dir.starts_with(root))
        {
            return;
        }
        *self.reveal_target.borrow_mut() = Some(dir.to_path_buf());
        self.advance_reveal();
    }

    /// Open the next folder toward the reveal target; each read calls this
    /// again until the target itself is open and selected.
    pub(super) fn advance_reveal(&self) {
        let Some(target) = self.reveal_target.borrow().clone() else {
            return;
        };
        let Some(root) = self.root_dir.borrow().clone() else {
            return;
        };
        let mut chain: Vec<&Path> = target
            .ancestors()
            .take_while(|p| *p != root.as_path())
            .collect();
        chain.reverse();
        for path in chain {
            let Some((_, row)) = self.find(path) else {
                return;
            };
            if row.is_expandable() && !row.is_expanded() {
                row.set_expanded(true);
            }
        }
        self.reveal_target.take();
        self.select_path(&target, true);
    }

    pub(super) fn find(&self, path: &Path) -> Option<(u32, gtk::TreeListRow)> {
        (0..self.tree.n_items()).find_map(|i| {
            let row = tree_row(self.tree.item(i))?;
            (row_path(&row).as_deref() == Some(path)).then_some((i, row))
        })
    }

    /// Mark the file shown in the active editor tab.
    pub(crate) fn mark_open(&self, path: Option<PathBuf>) {
        if *self.open_file.borrow() == path {
            return;
        }
        *self.open_file.borrow_mut() = path;
        for item in self.bound.borrow().iter() {
            draw_item(item, self.open_file.borrow().as_deref());
        }
    }

    /// JSON picture of the tree for scripts (`app.files-state`).
    pub(crate) fn snapshot(&self) -> String {
        let open = self.open_file.borrow().clone();
        let rows: Vec<serde_json::Value> = (0..self.tree.n_items().min(80))
            .filter_map(|i| tree_row(self.tree.item(i)))
            .map(|r| {
                let path = row_path(&r);
                serde_json::json!({
                    "name": path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
                    "depth": r.depth(),
                    "dir": r.is_expandable(),
                    "expanded": r.is_expanded(),
                    "open": open.is_some() && open == path,
                })
            })
            .collect();
        serde_json::json!({
            "rows": rows,
            "selected": self.selected_path().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
            "focused": crate::pane::focus_inside(&self.list),
        })
        .to_string()
    }

    /// A key by name (`up`, `down`, `left`, `right`, `enter`), for scripts.
    pub(crate) fn key(self: &Rc<Self>, name: &str) {
        let at = self.selection.selected();
        let none = at >= self.tree.n_items();
        match name {
            "down" => self.focus_index(if none { 0 } else { at + 1 }),
            "up" => self.focus_index(if none { 0 } else { at.saturating_sub(1) }),
            "right" => {
                self.step_in();
            }
            "left" => {
                self.step_out();
            }
            "enter" if !none => self.list.emit_by_name::<()>("activate", &[&at]),
            _ => {}
        }
        self.publish();
    }
}
