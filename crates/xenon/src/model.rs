//! The product state of the GTK frontend: workspaces, their pane trees, and the
//! per-terminal status. No widgets here; the views draw whatever this says.
//! Layout rules come from `xenon_core`'s pane tree.

use std::collections::HashMap;
use std::path::Path;

use xenon_core::{
    AttentionMap, ContentLayout, PaneContent, PaneLeaf, PaneNode, PaneTab, PaneTree, Registry,
    SessionState, TabId, TabState, TerminalActivity, WorkspaceId, WorkspaceRec, terminal_tab_label,
};

/// What a tab holds. Editor, diff, and image tabs are not drawn yet; they are
/// kept exactly as saved so a GTK session never drops them.
#[derive(Clone)]
pub(crate) enum TabKind {
    Terminal,
    /// A read-only diff of one changed file (path relative to the workspace).
    Diff {
        path: String,
    },
    Kept(TabState),
}

#[derive(Clone)]
pub(crate) struct Tab {
    pub id: TabId,
    pub kind: TabKind,
    /// Cleaned terminal title (see `clean_terminal_title`).
    pub title: String,
    /// Exit code once the shell has ended.
    pub exited: Option<i32>,
    /// An editor tab with unsaved changes.
    pub modified: bool,
    /// Short name of the phone whose grid this terminal has right now.
    pub phone_device: Option<String>,
}

impl PaneTab for Tab {
    fn tab_id(&self) -> TabId {
        self.id
    }
}

impl Tab {
    pub(crate) fn terminal(id: TabId) -> Self {
        Self {
            id,
            kind: TabKind::Terminal,
            title: "terminal".into(),
            exited: None,
            modified: false,
            phone_device: None,
        }
    }

    /// Chip text for the tab at `index` in its pane.
    pub(crate) fn label(&self, index: usize) -> String {
        match &self.kind {
            TabKind::Terminal => {
                let label = terminal_tab_label(&self.title, index);
                match &self.phone_device {
                    Some(device) => format!("{label} \u{b7} {device}"),
                    None => label,
                }
            }
            TabKind::Kept(TabState::Editor { path, .. }) => {
                let name = path
                    .file_name()
                    .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
                if self.modified {
                    format!("{name} \u{25cf}")
                } else {
                    name
                }
            }
            TabKind::Diff { path } => path.rsplit('/').next().unwrap_or(path).to_string(),
            TabKind::Kept(_) => "tab".into(),
        }
    }

    fn state(&self) -> Option<TabState> {
        match &self.kind {
            TabKind::Terminal => Some(TabState::Terminal {
                id: self.id,
                cwd: ".".into(),
                remote_session: None,
            }),
            TabKind::Diff { path } => Some(TabState::Diff {
                id: self.id,
                path: path.clone(),
            }),
            TabKind::Kept(state) => Some(state.clone()),
        }
    }
}

/// One registered workspace and its session.
pub(crate) struct Space {
    pub content: PaneContent<Tab>,
    /// The session as loaded. A workspace nobody has opened this run is saved
    /// back exactly like this.
    pub saved: SessionState,
    /// Terminals exist for this workspace (it was opened this run).
    pub live: bool,
}

pub(crate) struct Model {
    pub registry: Registry,
    pub spaces: HashMap<WorkspaceId, Space>,
    pub attention: AttentionMap,
    pub activity: HashMap<(WorkspaceId, TabId), TerminalActivity>,
    /// The window has keyboard focus; a tab the user can see is "viewed".
    pub window_active: bool,
    /// Which sidebar sections are folded away (saved in settings.json).
    pub folds: Folds,
}

/// A sidebar section that folds to its header.
#[derive(Clone, Copy)]
pub(crate) enum Fold {
    Workspaces,
    Files,
    Git,
}

/// Fold state of each sidebar section; `true` is folded.
#[derive(Clone, Copy, Default)]
pub(crate) struct Folds {
    pub workspaces: bool,
    pub files: bool,
    pub git: bool,
}

impl Folds {
    fn load() -> Self {
        let s = xenon_store::load_settings().unwrap_or_default();
        Self {
            workspaces: s.workspaces_collapsed,
            files: !s.files_open,
            git: s.git_collapsed,
        }
    }

    pub(crate) fn get(&self, fold: Fold) -> bool {
        match fold {
            Fold::Workspaces => self.workspaces,
            Fold::Files => self.files,
            Fold::Git => self.git,
        }
    }

    pub(crate) fn set(&mut self, fold: Fold, folded: bool) {
        match fold {
            Fold::Workspaces => self.workspaces = folded,
            Fold::Files => self.files = folded,
            Fold::Git => self.git = folded,
        }
    }

    pub(crate) fn save(&self) {
        let result = xenon_store::update_settings(|s| {
            s.workspaces_collapsed = self.workspaces;
            s.files_open = !self.files;
            s.git_collapsed = self.git;
        });
        if let Err(error) = result {
            eprintln!("[xg] save settings: {error}");
        }
    }
}

impl Model {
    /// Read the registry and every session from the current data dir.
    pub(crate) fn load() -> Self {
        let registry = xenon_store::load_registry().unwrap_or_else(|error| {
            eprintln!("[xg] registry: {error}");
            Registry::default()
        });
        let spaces = registry
            .workspaces
            .iter()
            .map(|rec| {
                let saved = xenon_store::load_session(rec.id).unwrap_or_default();
                let space = Space {
                    content: PaneContent::default(),
                    saved,
                    live: false,
                };
                (rec.id, space)
            })
            .collect();
        Self {
            registry,
            spaces,
            attention: AttentionMap::default(),
            activity: HashMap::new(),
            window_active: true,
            folds: Folds::load(),
        }
    }

    pub(crate) fn active(&self) -> Option<WorkspaceId> {
        let id = self.registry.active?.workspace;
        self.registry.workspace(id).map(|rec| rec.id)
    }

    pub(crate) fn rec(&self, id: WorkspaceId) -> Option<&WorkspaceRec> {
        self.registry.workspace(id)
    }

    /// Build the pane tree from the saved session the first time a workspace
    /// is opened. Terminal slots reopen as fresh shells.
    pub(crate) fn materialize(&mut self, id: WorkspaceId) {
        let Some(space) = self.spaces.get_mut(&id) else {
            return;
        };
        if space.live {
            return;
        }
        space.live = true;
        let layout = &space.saved.content;
        let root = layout.root.as_ref().map(live_node);
        let mut content = PaneContent {
            root,
            focused: layout.focused,
        };
        let leaves = content.leaf_ids();
        if content.focused.is_none_or(|f| !leaves.contains(&f)) {
            content.focused = leaves.first().copied();
        }
        space.content = content;
    }

    /// The session to write for `id`.
    pub(crate) fn snapshot(&self, id: WorkspaceId) -> Option<SessionState> {
        let space = self.spaces.get(&id)?;
        if !space.live {
            return Some(space.saved.clone());
        }
        let content: ContentLayout = space.content.snapshot(&|tab: &Tab| tab.state());
        Some(SessionState {
            content,
            ..space.saved.clone()
        })
    }

    /// Register a folder as a workspace (or reuse / reopen it) and return it.
    pub(crate) fn open_dir(&mut self, path: &Path) -> WorkspaceId {
        let root = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(rec) = self.registry.workspaces.iter().find(|r| r.root == root) {
            return rec.id;
        }
        let rec = match self
            .registry
            .closed_workspaces
            .iter()
            .position(|r| r.root == root)
        {
            Some(i) => self.registry.closed_workspaces.remove(i),
            None => WorkspaceRec::new(root),
        };
        let id = rec.id;
        let saved = xenon_store::load_session(id).unwrap_or_default();
        self.registry.workspaces.push(rec);
        self.spaces.insert(
            id,
            Space {
                content: PaneContent::default(),
                saved,
                live: false,
            },
        );
        id
    }

    pub(crate) fn activate(&mut self, id: WorkspaceId) -> bool {
        let Some(rec) = self.registry.workspace_mut(id) else {
            return false;
        };
        rec.touch_opened();
        self.registry.active = Some(xenon_core::Active { workspace: id });
        self.materialize(id);
        true
    }

    /// Drag-reorder: put `id` where `before` sits (`None`: the end of the open
    /// list). False when nothing moved.
    pub(crate) fn move_workspace(&mut self, id: WorkspaceId, before: Option<WorkspaceId>) -> bool {
        if before == Some(id) {
            return false;
        }
        let list = &mut self.registry.workspaces;
        let Some(from) = list.iter().position(|r| r.id == id) else {
            return false;
        };
        if before.is_none() && from + 1 == list.len() {
            return false;
        }
        let rec = list.remove(from);
        let to = before
            .and_then(|b| list.iter().position(|r| r.id == b))
            .unwrap_or(list.len());
        list.insert(to, rec);
        true
    }

    /// Move `id` `delta` places along the list; false at either end.
    pub(crate) fn shift_workspace(&mut self, id: WorkspaceId, delta: isize) -> bool {
        let list = &self.registry.workspaces;
        let Some(from) = list.iter().position(|r| r.id == id) else {
            return false;
        };
        let to = from as isize + delta;
        if to < 0 || to >= list.len() as isize {
            return false;
        }
        // The row now at `to` (moving up) or just after it (moving down).
        let before = list.get(to as usize + usize::from(delta > 0)).map(|r| r.id);
        self.move_workspace(id, before)
    }

    /// Move `id` to the closed list and pick the neighbour to show next.
    pub(crate) fn close_workspace(&mut self, id: WorkspaceId) -> Option<WorkspaceId> {
        let index = self.registry.workspaces.iter().position(|r| r.id == id)?;
        let rec = self.registry.workspaces.remove(index);
        self.registry
            .closed_workspaces
            .retain(|r| r.root != rec.root);
        self.registry.closed_workspaces.insert(0, rec);
        self.registry.closed_workspaces.truncate(50);
        self.spaces.remove(&id);
        self.attention.clear_workspace(id);
        self.activity.retain(|(ws, _), _| *ws != id);
        let next = self
            .registry
            .workspaces
            .get(index.min(self.registry.workspaces.len().saturating_sub(1)))
            .map(|r| r.id);
        self.registry.active = next.map(|workspace| xenon_core::Active { workspace });
        next
    }

    /// Id of the workspace `delta` rows from the active one, wrapping.
    pub(crate) fn workspace_offset(&self, delta: isize) -> Option<WorkspaceId> {
        let list = &self.registry.workspaces;
        let current = self
            .active()
            .and_then(|id| list.iter().position(|r| r.id == id))?;
        let n = list.len() as isize;
        let next = (current as isize + delta).rem_euclid(n);
        list.get(next as usize).map(|r| r.id)
    }
}

fn live_node(node: &PaneNode) -> PaneTree<Tab> {
    match node {
        PaneNode::Leaf(leaf) => {
            let tabs: Vec<Tab> = leaf
                .tabs
                .iter()
                .map(|state| match state {
                    TabState::Terminal { id, .. } => Tab::terminal(*id),
                    TabState::Diff { id, path } => Tab {
                        id: *id,
                        kind: TabKind::Diff { path: path.clone() },
                        title: String::new(),
                        exited: None,
                        modified: false,
                        phone_device: None,
                    },
                    TabState::Editor { id, .. } => Tab {
                        id: *id,
                        kind: TabKind::Kept(state.clone()),
                        title: String::new(),
                        exited: None,
                        modified: false,
                        phone_device: None,
                    },
                })
                .collect();
            let active = leaf.active.min(tabs.len().saturating_sub(1));
            PaneTree::Leaf(PaneLeaf {
                id: leaf.id,
                tabs,
                active,
                parked: leaf.parked,
            })
        }
        PaneNode::Split {
            axis,
            children,
            sizes,
        } => PaneTree::Split {
            axis: *axis,
            children: children.iter().map(live_node).collect(),
            sizes: sizes.clone(),
        },
    }
}
