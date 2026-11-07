//! Workspace commands: open, switch, close, and the dialogs that ask for a folder.

use std::rc::Rc;

use xenon_core::WorkspaceId;

use crate::app::App;

impl App {
    /// Make workspace `id` the visible one. Terminals start the first time.
    pub(crate) fn activate_workspace(self: &Rc<Self>, id: WorkspaceId) {
        if !self.model.borrow_mut().activate(id) {
            return;
        }
        if self.model.borrow().drawable(id) {
            self.sync(id);
        } else {
            self.show_unsupported(id);
        }
        self.stack.set_visible_child_name(&id.to_string());
        self.model.borrow_mut().dismiss_viewed();
        self.paint_all();
        self.save_registry();
        self.focus_terminal();
    }

    pub(crate) fn activate_workspace_index(self: &Rc<Self>, index: usize) {
        let id = self
            .model
            .borrow()
            .registry
            .workspaces
            .get(index)
            .map(|r| r.id);
        if let Some(id) = id {
            self.activate_workspace(id);
        }
    }

    pub(crate) fn step_workspace(self: &Rc<Self>, delta: isize) {
        let id = self.model.borrow().workspace_offset(delta);
        if let Some(id) = id {
            self.activate_workspace(id);
        }
    }

    /// Register `path` as a workspace (or reuse it), show it, and give a new
    /// one its first terminal.
    pub(crate) fn open_workspace(self: &Rc<Self>, path: &std::path::Path) {
        let id = self.model.borrow_mut().open_dir(path);
        self.sidebar.rebuild(&self.model.borrow());
        self.activate_workspace(id);
        let empty = self
            .model
            .borrow()
            .spaces
            .get(&id)
            .is_some_and(|s| s.live && s.content.root.is_none());
        if empty && self.model.borrow().drawable(id) {
            self.new_terminal();
        }
    }

    pub(crate) fn close_workspace(self: &Rc<Self>) {
        let Some(id) = self.model.borrow().active() else {
            return;
        };
        self.save_session(id);
        let next = self.model.borrow_mut().close_workspace(id);
        let keys: Vec<_> = self
            .terms
            .borrow()
            .keys()
            .filter(|(w, _)| *w == id)
            .copied()
            .collect();
        for key in keys {
            self.terms.borrow_mut().remove(&key);
        }
        self.bodies.borrow_mut().retain(|(w, _), _| *w != id);
        if let Some(slot) = self.slots.borrow_mut().remove(&id) {
            self.stack.remove(&slot.holder);
        }
        self.sidebar.rebuild(&self.model.borrow());
        match next {
            Some(next) => self.activate_workspace(next),
            None => {
                self.stack.set_visible_child_name("none");
                self.paint_all();
                self.save_registry();
            }
        }
    }
}
