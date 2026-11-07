//! Git on the app: where measured statuses land, the diff tab commands, and
//! the section's focus chord. Measuring itself runs in `git.rs`'s worker.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use xenon_core::WorkspaceId;

use crate::app::App;
use crate::model::TabKind;

impl App {
    /// Tell the worker which workspace folders to watch.
    pub(crate) fn sync_git_roots(&self) {
        let roots = self
            .model
            .borrow()
            .registry
            .workspaces
            .iter()
            .filter(|r| r.ssh.is_none())
            .map(|r| (r.id, r.root.clone()))
            .collect();
        self.git.set_roots(roots);
    }

    /// A batch of finished measurements from the worker.
    pub(crate) fn git_measured(
        self: &Rc<Self>,
        batch: Vec<(WorkspaceId, Option<xenon_git::status::GitStatus>)>,
    ) {
        let changed = crate::git::apply(&self.git, batch);
        if changed.is_empty() {
            return;
        }
        self.paint_all();
        self.publish_git();
        // Open diffs of a changed workspace read again; the file entry may
        // carry new counts.
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

    /// The window just got focus: look at the active workspace again.
    pub(crate) fn git_refresh_active(&self) {
        if let Some(ws) = self.model.borrow().active() {
            self.git.refresh(ws);
        }
    }

    /// File of the diff tab that is on screen in `ws`, relative to the workspace.
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

    /// Open (or show again) the diff of `path` in the focused pane.
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

    /// Open `path` in an editor tab with the cursor on `line`.
    pub(crate) fn open_file_at(
        self: &Rc<Self>,
        ws: WorkspaceId,
        path: &std::path::Path,
        line: u32,
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
            editor.goto_line(line);
        }
    }

    /// The Git section chord: focus it (opening it); pressed again while the
    /// cursor is in it, fold it away and go back to the pane.
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

    pub(crate) fn publish_git(&self) {
        let mut state = self.sidebar.git.snapshot();
        let model = self.model.borrow();
        let dirt: serde_json::Map<String, serde_json::Value> = model
            .registry
            .workspaces
            .iter()
            .map(|r| {
                let name = match self.git.dirt(r.id) {
                    xenon_git::status::DirtState::Clean => "clean",
                    xenon_git::status::DirtState::IndexOnly => "index",
                    xenon_git::status::DirtState::Dirty => "dirty",
                };
                (r.name.clone(), name.into())
            })
            .collect();
        state["dirt"] = dirt.into();
        drop(model);
        self.publish_to("git-state", &state.to_string());
    }

    pub(crate) fn publish_diff(&self) {
        let json = self
            .active_diff()
            .map_or_else(|| "{}".to_string(), |v| v.snapshot());
        self.publish_to("diff-state", &json);
    }
}
