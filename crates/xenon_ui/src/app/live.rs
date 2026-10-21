//! Runtime pane tree: mixed terminal/editor tabs per leaf.

use super::*;
use xenon_core::{ContentLayout, LeafPane, PaneId, PaneNode, SplitAxis, TabId, TabState};

/// One open surface in a leaf.
#[derive(Clone)]
pub(crate) enum LiveTab {
    Terminal {
        id: TabId,
        view: Entity<TerminalView>,
    },
    Editor {
        id: TabId,
        path: PathBuf,
        name: String,
        view: Entity<EditorView>,
    },
    /// Read-only git diff of one workspace-relative path. Not persisted.
    Diff {
        id: TabId,
        path: String,
        name: String,
        view: Entity<crate::diff_view::DiffView>,
    },
}

impl LiveTab {
    pub(crate) fn id(&self) -> TabId {
        match self {
            Self::Terminal { id, .. } | Self::Editor { id, .. } | Self::Diff { id, .. } => *id,
        }
    }

    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal { .. })
    }

    pub(crate) fn editor_path(&self) -> Option<&Path> {
        match self {
            Self::Editor { path, .. } => Some(path.as_path()),
            _ => None,
        }
    }

    pub(crate) fn as_terminal(&self) -> Option<&Entity<TerminalView>> {
        match self {
            Self::Terminal { view, .. } => Some(view),
            _ => None,
        }
    }

    pub(crate) fn as_editor(&self) -> Option<&Entity<EditorView>> {
        match self {
            Self::Editor { view, .. } => Some(view),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct LiveLeaf {
    pub id: PaneId,
    pub tabs: Vec<LiveTab>,
    pub active: usize,
    pub parked: bool,
}

impl LiveLeaf {
    pub(crate) fn active_tab(&self) -> Option<&LiveTab> {
        self.tabs.get(self.active)
    }
}

#[derive(Clone)]
pub(crate) enum LiveNode {
    Leaf(LiveLeaf),
    Split {
        axis: SplitAxis,
        children: Vec<LiveNode>,
        /// Fractions summing to 1; see `xenon_core::flatten_split`.
        sizes: Vec<f32>,
    },
}

impl LiveNode {
    pub(crate) fn for_each_leaf(&self, f: &mut dyn FnMut(&LiveLeaf)) {
        match self {
            Self::Leaf(leaf) => f(leaf),
            Self::Split { children, .. } => {
                for c in children {
                    c.for_each_leaf(f);
                }
            }
        }
    }

    pub(crate) fn leaf_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.for_each_leaf(&mut |leaf| out.push(leaf.id));
        out
    }

    pub(crate) fn find_leaf(&self, id: PaneId) -> Option<&LiveLeaf> {
        match self {
            Self::Leaf(leaf) if leaf.id == id => Some(leaf),
            Self::Leaf(_) => None,
            Self::Split { children, .. } => children.iter().find_map(|c| c.find_leaf(id)),
        }
    }

    pub(crate) fn find_leaf_mut(&mut self, id: PaneId) -> Option<&mut LiveLeaf> {
        match self {
            Self::Leaf(l) if l.id == id => Some(l),
            Self::Leaf(_) => None,
            Self::Split { children, .. } => children.iter_mut().find_map(|c| c.find_leaf_mut(id)),
        }
    }

    pub(crate) fn find_tab(&self, tab: TabId) -> Option<(PaneId, usize)> {
        let mut found = None;
        self.for_each_leaf(&mut |leaf| {
            if found.is_none() {
                found = leaf
                    .tabs
                    .iter()
                    .position(|t| t.id() == tab)
                    .map(|i| (leaf.id, i));
            }
        });
        found
    }

    pub(crate) fn find_editor_path(&self, path: &Path) -> Option<(PaneId, usize)> {
        let mut found = None;
        self.for_each_leaf(&mut |leaf| {
            if found.is_none() {
                found = leaf
                    .tabs
                    .iter()
                    .position(|t| t.editor_path() == Some(path))
                    .map(|i| (leaf.id, i));
            }
        });
        found
    }

    pub(crate) fn for_each_terminal(&self, f: &mut dyn FnMut(TabId, &Entity<TerminalView>)) {
        self.for_each_leaf(&mut |leaf| {
            for tab in &leaf.tabs {
                if let LiveTab::Terminal { id, view } = tab {
                    f(*id, view);
                }
            }
        });
    }

    pub(crate) fn for_each_editor(&self, f: &mut dyn FnMut(&Entity<EditorView>, &Path)) {
        self.for_each_leaf(&mut |leaf| {
            for tab in &leaf.tabs {
                if let LiveTab::Editor { view, path, .. } = tab {
                    f(view, path);
                }
            }
        });
    }

    fn max_ids(&self) -> (u64, u64) {
        let mut max_ids = (0, 0);
        self.for_each_leaf(&mut |leaf| {
            max_ids.0 = max_ids.0.max(leaf.id.0);
            for tab in &leaf.tabs {
                max_ids.1 = max_ids.1.max(tab.id().0);
            }
        });
        max_ids
    }
}

#[derive(Default, Clone)]
pub(crate) struct LiveContent {
    pub root: Option<LiveNode>,
    pub focused: Option<PaneId>,
}

impl LiveContent {
    pub(crate) fn focused_leaf(&self) -> Option<&LiveLeaf> {
        let id = self.focused?;
        self.root.as_ref()?.find_leaf(id)
    }

    pub(crate) fn focused_leaf_mut(&mut self) -> Option<&mut LiveLeaf> {
        let id = self.focused?;
        self.root.as_mut()?.find_leaf_mut(id)
    }

    pub(crate) fn active_tab(&self) -> Option<&LiveTab> {
        self.focused_leaf()?.active_tab()
    }

    pub(crate) fn leaf_ids(&self) -> Vec<PaneId> {
        self.root
            .as_ref()
            .map(LiveNode::leaf_ids)
            .unwrap_or_default()
    }

    pub(crate) fn next_pane_id(&self) -> PaneId {
        let max = self.root.as_ref().map(|r| r.max_ids().0).unwrap_or(0);
        PaneId(max + 1)
    }

    pub(crate) fn next_tab_id(&self) -> TabId {
        let max = self.root.as_ref().map(|r| r.max_ids().1).unwrap_or(0);
        TabId(max + 1)
    }

    pub(crate) fn has_editor(&self) -> bool {
        let mut any = false;
        if let Some(root) = &self.root {
            root.for_each_editor(&mut |_, _| any = true);
        }
        any
    }

    pub(crate) fn has_terminal(&self) -> bool {
        let mut any = false;
        if let Some(root) = &self.root {
            root.for_each_terminal(&mut |_, _| any = true);
        }
        any
    }

    pub(crate) fn snapshot(&self) -> ContentLayout {
        ContentLayout {
            root: self.root.as_ref().map(snapshot_node),
            focused: self.focused,
        }
    }

    pub(crate) fn unsplit_empty(&mut self, empty: PaneId) {
        let Some(root) = self.root.as_mut() else {
            return;
        };
        if matches!(root, LiveNode::Leaf(l) if l.id == empty) {
            self.root = None;
            self.focused = None;
            return;
        }
        if let Some(focus) = root.remove_leaf(empty) {
            root.normalize();
            self.focused = Some(focus);
        }
    }
}

fn snapshot_node(node: &LiveNode) -> PaneNode {
    match node {
        LiveNode::Leaf(leaf) => {
            // Diff tabs are not restored; keep the active index on saved tabs.
            let tabs: Vec<TabState> = leaf.tabs.iter().filter_map(snapshot_tab).collect();
            let kept_before = leaf.tabs[..leaf.active.min(leaf.tabs.len())]
                .iter()
                .filter(|tab| snapshot_tab(tab).is_some())
                .count();
            PaneNode::Leaf(LeafPane {
                id: leaf.id,
                active: kept_before.min(tabs.len().saturating_sub(1)),
                parked: leaf.parked || (tabs.is_empty() && !leaf.tabs.is_empty()),
                tabs,
            })
        }
        LiveNode::Split {
            axis,
            children,
            sizes,
        } => PaneNode::Split {
            axis: *axis,
            children: children.iter().map(snapshot_node).collect(),
            sizes: sizes.clone(),
        },
    }
}

fn snapshot_tab(tab: &LiveTab) -> Option<TabState> {
    Some(match tab {
        LiveTab::Terminal { id, .. } => TabState::Terminal {
            remote_session: None,
            id: *id,
            cwd: PathBuf::from("."),
        },
        LiveTab::Editor { id, path, .. } => TabState::Editor {
            id: *id,
            path: path.clone(),
            cursor: xenon_core::Point::default(),
            scroll_top: 0,
        },
        LiveTab::Diff { .. } => return None,
    })
}

/// Payload for dragging a tab between panes.
#[derive(Clone, Debug)]
pub(crate) struct DragTab {
    pub workspace: WorkspaceId,
    pub tab: TabId,
}

pub(crate) fn fix_active_idx(active: &mut usize, removed: usize, len: usize) {
    if *active > removed {
        *active -= 1;
    } else if *active >= len {
        *active = len.saturating_sub(1);
    }
}

mod split_tree;

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
