use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use xenon_finder::fileops;

use super::FilesView;
use super::ops::target_of;
use crate::app::App;

fn dropped_paths(value: &glib::Value) -> Vec<PathBuf> {
    value
        .get::<gdk::FileList>()
        .map(|list| list.files().iter().filter_map(gio::File::path).collect())
        .unwrap_or_default()
}

impl FilesView {
    pub(super) fn wire_drop(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.list.add_controller(drop_target(move |paths| {
            let this = weak.upgrade()?;
            let root = this.root_dir.borrow().clone()?;
            Some(this.dropped(paths, root))
        }));
    }

    pub(super) fn row_events(owner: &OwnerCell, expander: &gtk::TreeExpander) {
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::MOVE);
        let (weak_row, o) = (expander.downgrade(), owner.clone());
        // sync: must return the content provider; it only records the dragged path.
        source.connect_prepare(move |_, _, _| {
            let target = target_of(&weak_row.upgrade()?)?;
            *o.get()?.drag.borrow_mut() = vec![target.path.clone()];
            let list = gdk::FileList::from_array(&[gio::File::for_path(&target.path)]);
            Some(gdk::ContentProvider::for_value(&list.to_value()))
        });
        let o = owner.clone();
        source.connect_drag_end(move |_, _, _| {
            if let Some(this) = o.get() {
                this.drag.borrow_mut().clear();
            }
        });
        expander.add_controller(source);

        let (weak_row, o) = (expander.downgrade(), owner.clone());
        let drop = drop_target(move |paths| {
            let target = target_of(&weak_row.upgrade()?)?;
            let dest = fileops::new_item_dir(&target.path, target.is_dir);
            Some(o.get()?.dropped(paths, dest))
        });
        spring_open(&drop, expander);
        expander.add_controller(drop);
    }

    fn dropped(self: &Rc<Self>, paths: Vec<PathBuf>, dest: PathBuf) -> bool {
        if paths.is_empty() {
            return false;
        }
        let moving = *self.drag.borrow() == paths;
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(this) = weak.upgrade() {
                this.transfer(paths, dest, moving);
            }
        });
        true
    }

    fn transfer(self: &Rc<Self>, paths: Vec<PathBuf>, dest: PathBuf, moving: bool) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let mut last = None;
            let mut dirs = vec![dest.clone()];
            for src in paths {
                let planned = if moving {
                    fileops::move_target(&src, &dest)
                } else {
                    fileops::copy_target(&src, &dest).map(Some)
                };
                let target = match planned {
                    Ok(Some(target)) => target,
                    Ok(None) => continue,
                    Err(why) => {
                        if let Some(this) = weak.upgrade() {
                            this.say(&why);
                        }
                        continue;
                    }
                };
                let result = if moving {
                    let from = gio::File::for_path(&src);
                    let to = gio::File::for_path(&target);
                    from.move_future(&to, gio::FileCopyFlags::NONE, glib::Priority::DEFAULT)
                        .0
                        .await
                        .map_err(|e| e.message().to_string())
                } else {
                    let (from, to) = (src.clone(), target.clone());
                    match gio::spawn_blocking(move || xenon_finder::fsops::copy_tree(&from, &to))
                        .await
                    {
                        Ok(r) => r.map_err(|e| e.to_string()),
                        Err(_) => Err("the copy was interrupted".into()),
                    }
                };
                let Some(this) = weak.upgrade() else { return };
                match result {
                    Ok(()) => {
                        if moving {
                            if let Some(app) = this.app.upgrade() {
                                app.paths_moved(&src, &target);
                            }
                            dirs.extend(src.parent().map(Path::to_path_buf));
                        }
                        last = Some(target);
                    }
                    Err(why) => {
                        let name = src.file_name().unwrap_or_default().to_string_lossy();
                        this.say(&format!(
                            "Couldn\u{2019}t {} \u{201c}{name}\u{201d}: {why}",
                            if moving { "move" } else { "copy" }
                        ));
                    }
                }
            }
            if let Some(this) = weak.upgrade() {
                this.finish(&dirs, last);
            }
        });
    }

    pub(super) fn finish(self: &Rc<Self>, dirs: &[PathBuf], show: Option<PathBuf>) {
        for dir in dirs {
            let known = self.root_dir.borrow().as_deref() == Some(dir.as_path())
                || self.stores.borrow().contains_key(dir);
            if known {
                self.load(dir);
            }
        }
        if let Some(path) = show {
            self.reveal(&path);
        }
    }

    pub(super) fn say(&self, text: &str) {
        if let Some(app) = self.app.upgrade() {
            app.toast(text);
        }
    }
}

impl App {
    pub(crate) fn paths_moved(self: &Rc<Self>, old: &Path, new: &Path) {
        let moved = self.model.borrow_mut().follow_rename(old, new);
        for (ws, tab) in &moved {
            let key = (*ws, *tab);
            self.bodies.borrow_mut().remove(&key);
            self.editors.borrow_mut().remove(&key);
            self.images.borrow_mut().remove(&key);
        }
        let spaces: std::collections::HashSet<_> = moved.iter().map(|(ws, _)| *ws).collect();
        for ws in spaces {
            self.sync(ws);
            self.save_session(ws);
        }
    }
}

fn drop_target(drop: impl Fn(Vec<PathBuf>) -> Option<bool> + 'static) -> gtk::DropTarget {
    let target = gtk::DropTarget::new(
        gdk::FileList::static_type(),
        gdk::DragAction::MOVE | gdk::DragAction::COPY,
    );
    // sync: the drop handler must say now whether it took the files.
    target.connect_drop(move |_, value, _, _| drop(dropped_paths(value)).unwrap_or(false));
    target
}

fn spring_open(drop: &gtk::DropTarget, expander: &gtk::TreeExpander) {
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    let (t, weak) = (timer.clone(), expander.downgrade());
    // sync: must return the drag action; the expand itself runs from the timeout.
    drop.connect_enter(move |_, _, _| {
        if let Some(old) = t.borrow_mut().take() {
            old.remove();
        }
        let (t2, weak) = (t.clone(), weak.clone());
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
            t2.borrow_mut().take();
            if let Some(row) = weak.upgrade().and_then(|e| e.list_row())
                && row.is_expandable()
                && !row.is_expanded()
            {
                row.set_expanded(true);
            }
        });
        *t.borrow_mut() = Some(id);
        gdk::DragAction::empty()
    });
    let t = timer.clone();
    drop.connect_leave(move |_| {
        if let Some(id) = t.borrow_mut().take() {
            id.remove();
        }
    });
    drop.connect_drop(move |_, _, _, _| {
        if let Some(id) = timer.borrow_mut().take() {
            id.remove();
        }
        false
    });
}

#[derive(Clone, Default)]
pub(super) struct OwnerCell(pub Rc<RefCell<Weak<FilesView>>>);

impl OwnerCell {
    pub(super) fn get(&self) -> Option<Rc<FilesView>> {
        self.0.borrow().upgrade()
    }
}
