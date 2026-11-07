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
        }
    }

    /// Chip text for the tab at `index` in its pane.
    pub(crate) fn label(&self, index: usize) -> String {
        match &self.kind {
            TabKind::Terminal => terminal_tab_label(&self.title, index),
            TabKind::Kept(TabState::Editor { path, .. }) => path
                .file_name()
                .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned()),
            TabKind::Kept(_) => "tab".into(),
        }
    }

    fn state(&self) -> TabState {
        match &self.kind {
            TabKind::Terminal => TabState::Terminal {
                id: self.id,
                cwd: ".".into(),
                remote_session: None,
            },
            TabKind::Kept(state) => state.clone(),
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
        }
    }

    pub(crate) fn active(&self) -> Option<WorkspaceId> {
        let id = self.registry.active?.workspace;
        self.registry.workspace(id).map(|rec| rec.id)
    }

    pub(crate) fn rec(&self, id: WorkspaceId) -> Option<&WorkspaceRec> {
        self.registry.workspace(id)
    }

    /// GTK draws local terminals only; SSH workspaces keep their saved session.
    pub(crate) fn drawable(&self, id: WorkspaceId) -> bool {
        self.rec(id).is_some_and(|rec| rec.ssh.is_none())
    }

    /// Build the pane tree from the saved session the first time a workspace
    /// is opened. Terminal slots reopen as fresh shells, like the GPUI app.
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
        let content: ContentLayout = space.content.snapshot(&|tab: &Tab| Some(tab.state()));
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
        if self.drawable(id) {
            self.materialize(id);
        }
        true
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
            let mut tabs: Vec<Tab> = leaf
                .tabs
                .iter()
                .map(|state| match state {
                    TabState::Terminal { id, .. } => Tab::terminal(*id),
                    TabState::Editor { id, .. } => Tab {
                        id: *id,
                        kind: TabKind::Kept(state.clone()),
                        title: String::new(),
                        exited: None,
                    },
                })
                .collect();
            if tabs.is_empty() && !leaf.parked {
                tabs.push(Tab::terminal(TabId(leaf.id.0.saturating_mul(1000) + 1)));
            }
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
