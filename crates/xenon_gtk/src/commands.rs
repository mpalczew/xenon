//! Commands: what a key, button, or D-Bus action asks the app to do. Each one
//! asks the model, then syncs the widgets and saves.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use xenon_core::{PaneId, ParkOutcome, SplitAxis, TabId, WorkspaceId};

use crate::app::App;
use crate::model_tabs::SplitResult;

impl App {
    pub(crate) fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    /// Put the cursor in the active tab once the widgets have settled.
    pub(crate) fn focus_terminal(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(app) = weak.upgrade() else { return };
            let key = {
                let model = app.model.borrow();
                let ws = model.active();
                let tab = ws
                    .and_then(|w| model.spaces.get(&w))
                    .and_then(|s| s.content.active_tab());
                ws.zip(tab.map(|t| t.id))
            };
            let Some(key) = key else { return };
            if let Some(view) = app.terms.borrow().get(&key).cloned() {
                view.term.grab_focus();
            } else if let Some(editor) = app.editors.borrow().get(&key).cloned() {
                editor.view.grab_focus();
            } else if let Some(diff) = app.diffs.borrow().get(&key).cloned() {
                diff.focus();
            }
            app.publish_git();
        });
    }

    /// Show `path` in an editor tab of `ws`; files this tab cannot show open in
    /// the default app.
    pub(crate) fn open_editor(self: &Rc<Self>, ws: WorkspaceId, path: &std::path::Path) {
        if let Err(why) = crate::editor::check(path) {
            self.toast(&format!("Opening {} outside Xenon: {why}", path.display()));
            self.launch(&gio::File::for_path(path).uri());
            return;
        }
        if self.model.borrow().active() != Some(ws) {
            self.activate_workspace(ws);
        }
        if self.model.borrow_mut().open_editor(ws, path).is_some() {
            self.touch_recent(ws, path);
            self.changed(ws);
        }
    }

    /// Show a folder of the workspace in the Files tree.
    pub(crate) fn reveal_dir(self: &Rc<Self>, _ws: WorkspaceId, dir: &std::path::Path) {
        self.sidebar.root.set_visible(true);
        self.sidebar.files.reveal(dir);
    }

    pub(crate) fn changed(self: &Rc<Self>, ws: WorkspaceId) {
        self.sync(ws);
        self.save_session(ws);
        self.focus_terminal();
    }

    pub(crate) fn new_terminal(self: &Rc<Self>) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        self.model.borrow_mut().new_terminal(ws);
        self.changed(ws);
    }

    pub(crate) fn new_terminal_in(self: &Rc<Self>, ws: WorkspaceId, pane: PaneId) {
        if let Some(space) = self.model.borrow_mut().spaces.get_mut(&ws) {
            space.content.focused = Some(pane);
        }
        self.model.borrow_mut().new_terminal(ws);
        self.changed(ws);
    }

    pub(crate) fn split(self: &Rc<Self>, axis: SplitAxis) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        let result = self.model.borrow_mut().split(ws, axis);
        match result {
            SplitResult::Done => self.changed(ws),
            SplitResult::NoRoom => self.toast("No room for another split"),
            SplitResult::Nothing => {}
        }
    }

    pub(crate) fn park_pane(self: &Rc<Self>) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        let outcome = self.model.borrow_mut().park_pane(ws);
        match outcome {
            ParkOutcome::NoRoom => self.toast("No room for another split"),
            ParkOutcome::Unchanged => {}
            _ => self.changed(ws),
        }
    }

    pub(crate) fn remove_empty_pane(self: &Rc<Self>, ws: WorkspaceId, pane: PaneId) {
        if self.model.borrow_mut().remove_empty_pane(ws, pane) {
            self.changed(ws);
        }
    }

    pub(crate) fn close_tab_ui(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        self.model.borrow_mut().close_tab(ws, tab);
        self.changed(ws);
    }

    pub(crate) fn close_active_tab(self: &Rc<Self>) {
        let model = self.model.borrow();
        let ws = model.active();
        let tab = ws
            .and_then(|w| model.spaces.get(&w))
            .and_then(|s| s.content.active_tab())
            .map(|t| t.id);
        drop(model);
        if let (Some(ws), Some(tab)) = (ws, tab) {
            self.close_tab_ui(ws, tab);
        }
    }

    pub(crate) fn close_other_tabs(self: &Rc<Self>) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        let others = self.model.borrow().other_tabs(ws);
        for tab in others {
            self.model.borrow_mut().close_tab(ws, tab);
        }
        self.changed(ws);
    }

    pub(crate) fn select_tab_ui(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        self.model.borrow_mut().select_tab(ws, tab);
        self.model.borrow_mut().dismiss_viewed();
        self.sync(ws);
        self.focus_terminal();
    }

    pub(crate) fn step_tab(self: &Rc<Self>, delta: isize) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        self.model.borrow_mut().step_tab(ws, delta);
        self.model.borrow_mut().dismiss_viewed();
        self.sync(ws);
        self.focus_terminal();
    }

    pub(crate) fn next_pane(self: &Rc<Self>) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        self.model.borrow_mut().step_pane(ws, 1);
        self.model.borrow_mut().dismiss_viewed();
        self.sync(ws);
        self.focus_terminal();
    }

    pub(crate) fn divider_moved(self: &Rc<Self>, ws: WorkspaceId, key: PaneId, cumulative: f32) {
        if self.model.borrow_mut().set_divider(ws, key, cumulative) {
            self.schedule_save();
        }
    }

    pub(crate) fn toggle_sidebar(&self) {
        let shown = self.sidebar.root.is_visible();
        self.sidebar.root.set_visible(!shown);
    }

    pub(crate) fn focus_sidebar(&self) {
        self.sidebar.root.set_visible(true);
        if self.sidebar.files.has_rows() {
            self.sidebar.files.focus();
            return;
        }
        let active = self.model.borrow().active();
        let index = active.and_then(|id| {
            self.model
                .borrow()
                .registry
                .workspaces
                .iter()
                .position(|r| r.id == id)
        });
        if let Some(row) = self.sidebar.list.row_at_index(index.unwrap_or(0) as i32) {
            row.grab_focus();
        }
    }

    pub(crate) fn zoom(self: &Rc<Self>, delta: f32) {
        {
            let mut settings = self.settings.borrow_mut();
            if delta == 0.0 {
                settings.reset_font_sizes();
            } else {
                settings.nudge_terminal_font_size(delta);
            }
            settings.persist();
        }
        for view in self.terms.borrow().values() {
            self.restyle_term(view);
        }
    }

    /// The terminal with the cursor, for copy, paste, and find.
    pub(crate) fn current_term(&self) -> Option<Rc<crate::term::TermView>> {
        let model = self.model.borrow();
        let ws = model.active()?;
        let tab = model.spaces.get(&ws)?.content.active_tab()?.id;
        self.terms.borrow().get(&(ws, tab)).cloned()
    }
}
