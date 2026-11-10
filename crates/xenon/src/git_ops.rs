use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use xenon_core::WorkspaceId;

use crate::app::App;
use crate::model::TabKind;

impl App {
    pub(crate) fn sync_git_roots(&self) {
        let roots = self
            .model
            .borrow()
            .registry
            .workspaces
            .iter()
            .map(|r| (r.id, r.root.clone()))
            .collect();
        self.git.set_roots(roots);
    }

    pub(crate) fn git_measured(
        self: &Rc<Self>,
        batch: Vec<(WorkspaceId, Option<xenon_git::status::GitStatus>)>,
    ) {
        let changed = crate::git::apply(&self.git, batch);
        if changed.is_empty() {
            return;
        }
        self.paint_all();
        for ws in changed {
            let views: Vec<_> = self
                .diffs
                .borrow()
                .iter()
                .filter(|((w, _), _)| *w == ws)
                .map(|(_, v)| v.clone())
                .collect();
            let status = self.git.status_of(ws);
            for view in views {
                let file = status
                    .as_ref()
                    .and_then(|s| s.files.iter().find(|f| f.path == view.path).cloned());
                view.reload(file);
            }
        }
    }

    pub(crate) fn git_refresh_active(&self) {
        if let Some(ws) = self.model.borrow().active() {
            self.git.refresh(ws);
        }
    }

    pub(crate) fn shown_diff_in(
        &self,
        model: &crate::model::Model,
        ws: WorkspaceId,
    ) -> Option<String> {
        let tab = model.spaces.get(&ws)?.content.active_tab()?;
        match &tab.kind {
            TabKind::Diff { path } => Some(path.clone()),
            _ => None,
        }
    }

    pub(crate) fn open_diff(self: &Rc<Self>, path: &str) {
        let Some(ws) = self.model.borrow().active() else {
            return;
        };
        let Some(file) = self
            .git
            .status_of(ws)
            .and_then(|s| s.files.into_iter().find(|f| f.path == path))
        else {
            return;
        };
        let opened = self.model.borrow_mut().open_diff(ws, path);
        let Some((tab, is_new)) = opened else { return };
        self.sync(ws);
        self.save_session(ws);
        let view = self.diffs.borrow().get(&(ws, tab)).cloned();
        if let Some(view) = view {
            if !is_new {
                view.reload(Some(file));
            }
            let weak = Rc::downgrade(self);
            glib::idle_add_local_once(move || {
                if weak.upgrade().is_some() {
                    view.focus();
                }
            });
        }
    }

    pub(crate) fn open_file_at(
        self: &Rc<Self>,
        ws: WorkspaceId,
        path: &std::path::Path,
        line: u32,
        column: u32,
    ) {
        if !path.exists() {
            return;
        }
        self.open_editor(ws, path);
        let tab = self
            .model
            .borrow()
            .spaces
            .get(&ws)
            .and_then(|s| s.content.active_tab())
            .map(|t| t.id);
        let editor = tab.and_then(|t| self.editors.borrow().get(&(ws, t)).cloned());
        if let Some(editor) = editor {
            editor.goto_line(line, column);
        }
    }

    pub(crate) fn focus_git(self: &Rc<Self>) {
        self.sidebar.root.set_visible(true);
        let git = &self.sidebar.git;
        if git.has_focus() {
            git.fold();
            self.focus_terminal();
        } else {
            git.focus();
        }
    }

    pub(crate) fn cycle_diff_layout(&self) {
        if let Some(view) = self.active_diff() {
            view.cycle_layout();
        }
    }

    pub(crate) fn active_diff(&self) -> Option<Rc<crate::diff_view::DiffView>> {
        let model = self.model.borrow();
        let ws = model.active()?;
        let tab = model.spaces.get(&ws)?.content.active_tab()?.id;
        self.diffs.borrow().get(&(ws, tab)).cloned()
    }
}
