//! The app object: widgets plus the model, and the one `sync` that makes the
//! widgets match the model. Commands live in `commands.rs`, terminal signals in
//! `events.rs`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use adw::prelude::*;
use gtk::glib;
use xenon_core::{PaneId, TabId, TabState, WorkspaceId};
use xenon_settings::UiSettings;

use crate::diff_view::DiffView;
use crate::editor::EditorView;
use crate::git::GitHub;
use crate::layout;
use crate::model::{Model, TabKind};
use crate::palette::Palette as Launcher;
use crate::pane::PaneView;
use crate::sidebar::Sidebar;
use crate::term::TermView;

/// Widgets of one workspace: its root holder and its panes.
pub(crate) struct Slot {
    pub holder: gtk::Box,
    pub panes: HashMap<PaneId, Rc<PaneView>>,
    pub shape: String,
    /// True while we set divider positions ourselves.
    pub busy: Rc<Cell<bool>>,
}

pub(crate) struct App {
    pub gtk: adw::Application,
    pub win: adw::ApplicationWindow,
    pub model: RefCell<Model>,
    pub settings: RefCell<UiSettings>,
    /// Terminal face that exists on this machine (the saved one may not).
    pub mono: RefCell<String>,
    pub css: gtk::CssProvider,
    pub sidebar: Sidebar,
    pub split: gtk::Paned,
    pub stack: gtk::Stack,
    pub toasts: adw::ToastOverlay,
    pub title: adw::WindowTitle,
    pub slots: RefCell<HashMap<WorkspaceId, Slot>>,
    pub terms: RefCell<HashMap<(WorkspaceId, TabId), Rc<TermView>>>,
    pub bodies: RefCell<HashMap<(WorkspaceId, TabId), gtk::Widget>>,
    pub launcher: Rc<Launcher>,
    /// Most recently opened files per workspace (relative paths, newest first).
    pub recents: RefCell<HashMap<WorkspaceId, Vec<std::path::PathBuf>>>,
    /// Last file index built per workspace, so the finder opens with results.
    pub file_indexes: RefCell<HashMap<WorkspaceId, std::sync::Arc<xenon_finder::FileIndex>>>,
    pub editors: RefCell<HashMap<(WorkspaceId, TabId), Rc<EditorView>>>,
    pub diffs: RefCell<HashMap<(WorkspaceId, TabId), Rc<DiffView>>>,
    pub git: GitHub,
    pub pulse_source: RefCell<Option<glib::SourceId>>,
    /// Last pulse step drawn; -1 after a repaint reset the rails.
    pub pulse_level: Cell<i32>,
    pub started: Instant,
    /// True while `sync` changes the tab views, so their signals stay silent.
    pub tab_sync: Cell<bool>,
    /// A tab drag ended outside every bar and pane.
    pub refused_drag: Cell<bool>,
    /// A tab dropped on a pane body, waiting for the bar to finish its drag.
    pub pending_drop: Cell<Option<(PaneId, TabId, Option<xenon_core::DropEdge>)>>,
    pub save_pending: Cell<bool>,
    /// Header Save button; shown while the active editor has unsaved changes.
    pub save_button: gtk::Button,
    pub remote: crate::remote::Remote,
    pub tasks: crate::tasks::Tasks,
    pub trust_card: crate::tasks_ui::TrustCard,
    pub worklist: crate::worklist::Worklist,
}

impl App {
    /// Bring the widgets of `ws` in line with its model: terminals, panes,
    /// tabs, and the split layout.
    pub(crate) fn sync(self: &Rc<Self>, ws: WorkspaceId) {
        let Some(content) = self
            .model
            .borrow()
            .spaces
            .get(&ws)
            .map(|s| s.content.clone())
        else {
            return;
        };
        let was = self.tab_sync.replace(true);
        self.sync_inner(ws, &content);
        self.tab_sync.set(was);
        self.paint_all();
    }

    fn sync_inner(
        self: &Rc<Self>,
        ws: WorkspaceId,
        content: &xenon_core::PaneContent<crate::model::Tab>,
    ) {
        self.ensure_slot(ws);
        self.sync_bodies(ws, content);
        let ids = content.leaf_ids();
        let bodies = self.bodies_of(ws);
        let mut slots = self.slots.borrow_mut();
        let Some(slot) = slots.get_mut(&ws) else {
            return;
        };
        slot.panes.retain(|id, _| ids.contains(id));
        for id in &ids {
            slot.panes
                .entry(*id)
                .or_insert_with(|| PaneView::new(self, ws, *id));
        }
        let multi = ids.len() > 1;
        if let Some(root) = content.root.as_ref() {
            root.for_each_leaf(&mut |leaf| {
                if let Some(pane) = slot.panes.get(&leaf.id) {
                    pane.set_multi(multi);
                    pane.sync(self, leaf, &bodies);
                }
            });
        }
        let shape = content.root.as_ref().map(layout::shape).unwrap_or_default();
        if shape != slot.shape || slot.holder.first_child().is_none() {
            while let Some(old) = slot.holder.first_child() {
                layout::detach(&old);
                slot.holder.remove(&old);
            }
            let widget = match content.root.as_ref() {
                Some(root) => {
                    let ctx = layout::Ctx {
                        app: self,
                        ws,
                        panes: &slot.panes,
                        busy: &slot.busy,
                    };
                    layout::build(&ctx, root)
                }
                None => crate::empty::all_clear(self, true),
            };
            widget.set_vexpand(true);
            slot.holder.append(&widget);
            slot.shape = shape;
        }
    }

    pub(crate) fn ensure_slot(&self, ws: WorkspaceId) {
        let mut slots = self.slots.borrow_mut();
        if slots.contains_key(&ws) {
            return;
        }
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        self.stack.add_named(&holder, Some(&ws.to_string()));
        slots.insert(
            ws,
            Slot {
                holder,
                panes: HashMap::new(),
                shape: String::new(),
                busy: Rc::new(Cell::new(false)),
            },
        );
    }

    /// Create bodies for new tabs and drop the ones whose tab is gone.
    fn sync_bodies(
        self: &Rc<Self>,
        ws: WorkspaceId,
        content: &xenon_core::PaneContent<crate::model::Tab>,
    ) {
        let mut wanted = Vec::new();
        let cwd = self
            .model
            .borrow()
            .rec(ws)
            .map(|r| r.root.clone())
            .unwrap_or_default();
        if let Some(root) = content.root.as_ref() {
            root.for_each_leaf(&mut |leaf| wanted.extend(leaf.tabs.iter().cloned()));
        }
        let stale: Vec<(WorkspaceId, TabId)> = self
            .bodies
            .borrow()
            .keys()
            .filter(|(w, t)| *w == ws && !wanted.iter().any(|tab| tab.id == *t))
            .copied()
            .collect();
        for key in stale {
            self.bodies.borrow_mut().remove(&key);
            self.terms.borrow_mut().remove(&key);
            self.task_tab_gone(key.0, key.1);
            self.editors.borrow_mut().remove(&key);
            self.remote_tab_gone(key.0, key.1);
            self.diffs.borrow_mut().remove(&key);
        }
        for tab in wanted {
            let key = (ws, tab.id);
            if self.bodies.borrow().contains_key(&key) {
                continue;
            }
            let body: gtk::Widget = match &tab.kind {
                TabKind::Terminal => {
                    let view = TermView::new(self, ws, tab.id, &cwd);
                    self.restyle_term(&view);
                    match self.tasks.take_spawn(ws, tab.id) {
                        Some(task) => view.spawn_command(&task.cwd, &task.argv, &task.env),
                        None => view.spawn(&cwd),
                    }
                    let root = view.root.clone().upcast();
                    self.terms.borrow_mut().insert(key, view);
                    root
                }
                TabKind::Kept(TabState::Editor { path, .. }) => {
                    let editor = EditorView::new(self, ws, tab.id, path);
                    let root = editor.root.clone();
                    self.editors.borrow_mut().insert(key, editor);
                    root
                }
                TabKind::Diff { path } => {
                    let file = self
                        .git
                        .status_of(ws)
                        .and_then(|s| s.files.into_iter().find(|f| &f.path == path))
                        .unwrap_or_else(|| xenon_git::status::GitFile {
                            index: ' ',
                            worktree: 'M',
                            path: path.clone(),
                            from: None,
                            added: 0,
                            removed: 0,
                        });
                    let view = DiffView::new(self, ws, tab.id, cwd.clone(), file);
                    let root = view.root.clone().upcast();
                    self.diffs.borrow_mut().insert(key, view);
                    root
                }
                TabKind::Kept(state) => crate::empty::placeholder(state),
            };
            self.bodies.borrow_mut().insert(key, body);
        }
    }

    fn bodies_of(&self, ws: WorkspaceId) -> HashMap<TabId, gtk::Widget> {
        self.bodies
            .borrow()
            .iter()
            .filter(|((w, _), _)| *w == ws)
            .map(|((_, t), b)| (*t, b.clone()))
            .collect()
    }

    pub(crate) fn restyle_term(&self, view: &TermView) {
        let (family, size) = {
            let s = self.settings.borrow();
            (self.mono.borrow().clone(), s.terminal.size)
        };
        view.restyle(&family, size, crate::theme::dark());
    }

    /// The full path a tab stands for: its file, or a terminal's folder.
    fn tab_full_path(
        &self,
        model: &Model,
        ws: WorkspaceId,
        tab: &crate::model::Tab,
    ) -> Option<String> {
        let rec = model.rec(ws)?;
        match &tab.kind {
            TabKind::Kept(TabState::Editor { path, .. }) => Some(path.display().to_string()),
            TabKind::Diff { path } => Some(rec.root.join(path).display().to_string()),
            TabKind::Terminal => self
                .terms
                .borrow()
                .get(&(ws, tab.id))
                .map(|v| v.live_cwd().display().to_string()),
            _ => None,
        }
    }

    /// Everything that shows status or selection, repainted from the model.
    pub(crate) fn paint_all(self: &Rc<Self>) {
        let model = self.model.borrow();
        self.sidebar.paint(&model, &self.git);
        if let Some(ws) = model.active() {
            if let Some(space) = model.spaces.get(&ws) {
                let slots = self.slots.borrow();
                if let (Some(root), Some(slot)) = (space.content.root.as_ref(), slots.get(&ws)) {
                    root.for_each_leaf(&mut |leaf| {
                        let Some(pane) = slot.panes.get(&leaf.id) else {
                            return;
                        };
                        for (index, tab) in leaf.tabs.iter().enumerate() {
                            let dot = model.tab_dot(ws, tab.id);
                            let path = self.tab_full_path(&model, ws, tab);
                            let dot = if index == leaf.active { None } else { dot };
                            pane.paint(tab, index, dot, path);
                        }
                    });
                }
            }
            let name = model.rec(ws).map(|r| r.name.clone()).unwrap_or_default();
            let local_root = model.rec(ws).map(|r| r.root.clone());
            let open_file = model
                .spaces
                .get(&ws)
                .and_then(|s| s.content.active_tab())
                .and_then(|t| match &t.kind {
                    TabKind::Kept(TabState::Editor { path, .. }) => Some(path.clone()),
                    _ => None,
                });
            let local_root_is_some = local_root.is_some();
            self.sidebar.files.set_root(local_root);
            self.sidebar.files.mark_open(open_file);
            let status = self.git.status_of(ws).filter(|_| local_root_is_some);
            let root = model.rec(ws).map(|r| r.root.clone()).unwrap_or_default();
            self.sidebar.git.set(
                Some(ws),
                status.as_ref(),
                self.shown_diff_in(&model, ws),
                &root,
            );
            self.title.set_title(&name);
            let label = space_label(&model, ws);
            self.title.set_subtitle(&label);
            self.win.set_title(Some(&format!("{name} — Xenon")));
        } else {
            self.sidebar.files.set_root(None);
            self.sidebar
                .git
                .set(None, None, None, std::path::Path::new(""));
            self.title.set_title("Xenon");
            self.title.set_subtitle("");
        }
        drop(model);
        self.refresh_save_button();
        self.sync_git_roots();
        self.publish_state();
        self.remote_dots();
        self.publish_diff();
        self.pulse_level.set(-1);
        self.ensure_pulse();
    }
}

/// Label of the focused pane's active tab, for the header subtitle.
pub(crate) fn space_label(model: &crate::model::Model, ws: WorkspaceId) -> String {
    model
        .spaces
        .get(&ws)
        .and_then(|s| s.content.focused_leaf())
        .and_then(|leaf| leaf.active_tab().map(|t| t.label(leaf.active)))
        .unwrap_or_default()
}
