use std::rc::Rc;

use xenon_core::WorkspaceId;

use crate::app::App;

impl App {
    pub(crate) fn activate_workspace(self: &Rc<Self>, id: WorkspaceId) {
        if !self.model.borrow_mut().activate(id) {
            return;
        }
        self.sync(id);
        self.check_folder_open_tasks(id);
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

    pub(crate) fn reorder_workspace(self: &Rc<Self>, from: usize, to: Option<usize>) {
        let moved = {
            let mut model = self.model.borrow_mut();
            let list = &model.registry.workspaces;
            let id = list.get(from).map(|r| r.id);
            let before = to.and_then(|i| list.get(i)).map(|r| r.id);
            id.is_some_and(|id| model.move_workspace(id, before))
        };
        if moved {
            self.redraw_workspaces();
        }
    }

    pub(crate) fn shift_workspace(self: &Rc<Self>, delta: isize) {
        let moved = {
            let mut model = self.model.borrow_mut();
            model
                .active()
                .is_some_and(|id| model.shift_workspace(id, delta))
        };
        if moved {
            self.redraw_workspaces();
        }
    }

    fn redraw_workspaces(self: &Rc<Self>) {
        self.sidebar.rebuild(&self.model.borrow());
        self.paint_all();
        self.save_registry();
    }

    pub(crate) fn step_workspace(self: &Rc<Self>, delta: isize) {
        let id = self.model.borrow().workspace_offset(delta);
        if let Some(id) = id {
            self.activate_workspace(id);
        }
    }

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
        if empty {
            self.new_terminal();
        }
    }

    pub(crate) fn close_workspace(self: &Rc<Self>) {
        let active = self.model.borrow().active();
        if let Some(id) = active {
            self.close_workspace_id(id);
        }
    }

    pub(crate) fn close_workspace_id(self: &Rc<Self>, id: xenon_core::WorkspaceId) {
        self.save_session(id);
        let previous = self.model.borrow().active();
        let mut next = self.model.borrow_mut().close_workspace(id);
        if let Some(prev) = previous.filter(|p| *p != id) {
            next = Some(prev);
        }
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
