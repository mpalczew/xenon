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
}

impl LiveTab {
    pub(crate) fn id(&self) -> TabId {
        match self {
            Self::Terminal { id, .. } | Self::Editor { id, .. } => *id,
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
        ratio: f32,
        first: Box<LiveNode>,
        second: Box<LiveNode>,
    },
}

impl LiveNode {
    pub(crate) fn for_each_leaf(&self, f: &mut dyn FnMut(&LiveLeaf)) {
        match self {
            Self::Leaf(leaf) => f(leaf),
            Self::Split { first, second, .. } => {
                first.for_each_leaf(f);
                second.for_each_leaf(f);
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
            Self::Split { first, second, .. } => {
                first.find_leaf(id).or_else(|| second.find_leaf(id))
            }
        }
    }

    pub(crate) fn find_leaf_mut(&mut self, id: PaneId) -> Option<&mut LiveLeaf> {
        match self {
            Self::Leaf(l) if l.id == id => Some(l),
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => {
                first.find_leaf_mut(id).or_else(|| second.find_leaf_mut(id))
            }
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

    pub(crate) fn nest_depth(&self, leaf: PaneId) -> Option<u32> {
        self.nest_depth_inner(leaf, 0)
    }

    fn nest_depth_inner(&self, leaf: PaneId, depth: u32) -> Option<u32> {
        match self {
            Self::Leaf(l) if l.id == leaf => Some(depth),
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => first
                .nest_depth_inner(leaf, depth + 1)
                .or_else(|| second.nest_depth_inner(leaf, depth + 1)),
        }
    }

    /// Set ratio on the split whose first child is the leaf `first_child_leaf`.
    pub(crate) fn set_ratio_for_split(&mut self, first_child_leaf: PaneId, ratio: f32) -> bool {
        match self {
            Self::Leaf(_) => false,
            Self::Split {
                ratio: r,
                first,
                second,
                ..
            } => {
                if matches!(first.as_ref(), LiveNode::Leaf(l) if l.id == first_child_leaf) {
                    *r = ratio.clamp(0.15, 0.85);
                    return true;
                }
                first.set_ratio_for_split(first_child_leaf, ratio)
                    || second.set_ratio_for_split(first_child_leaf, ratio)
            }
        }
    }

    pub(crate) fn replace_leaf(&mut self, id: PaneId, replacement: LiveNode) -> bool {
        match self {
            Self::Leaf(l) if l.id == id => {
                *self = replacement;
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                if first.replace_leaf(id, replacement.clone()) {
                    true
                } else {
                    second.replace_leaf(id, replacement)
                }
            }
        }
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
    pub(crate) fn is_empty(&self) -> bool {
        self.root.is_none()
    }

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
        let Some(root) = self.root.take() else {
            return;
        };
        match unsplit_live(root, empty) {
            Ok((new_root, focus)) => {
                self.root = new_root;
                self.focused = focus.or_else(|| self.leaf_ids().first().copied());
            }
            Err(root) => self.root = Some(root),
        }
    }
}

fn snapshot_node(node: &LiveNode) -> PaneNode {
    match node {
        LiveNode::Leaf(leaf) => PaneNode::Leaf(LeafPane {
            id: leaf.id,
            active: leaf.active,
            tabs: leaf.tabs.iter().map(snapshot_tab).collect(),
        }),
        LiveNode::Split {
            axis,
            ratio,
            first,
            second,
        } => PaneNode::Split {
            axis: *axis,
            ratio: *ratio,
            first: Box::new(snapshot_node(first)),
            second: Box::new(snapshot_node(second)),
        },
    }
}

fn snapshot_tab(tab: &LiveTab) -> TabState {
    match tab {
        LiveTab::Terminal { id, .. } => TabState::Terminal {
            id: *id,
            cwd: PathBuf::from("."),
        },
        LiveTab::Editor { id, path, .. } => TabState::Editor {
            id: *id,
            path: path.clone(),
            cursor: xenon_core::Point::default(),
            scroll_top: 0,
        },
    }
}

fn unsplit_live(
    node: LiveNode,
    empty: PaneId,
) -> Result<(Option<LiveNode>, Option<PaneId>), LiveNode> {
    match node {
        LiveNode::Leaf(l) if l.id == empty => Ok((None, None)),
        LiveNode::Leaf(l) => Err(LiveNode::Leaf(l)),
        LiveNode::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            if matches!(first.as_ref(), LiveNode::Leaf(l) if l.id == empty) {
                let focus = second.leaf_ids().first().copied();
                return Ok((Some(*second), focus));
            }
            if matches!(second.as_ref(), LiveNode::Leaf(l) if l.id == empty) {
                let focus = first.leaf_ids().first().copied();
                return Ok((Some(*first), focus));
            }
            match unsplit_live(*first, empty) {
                Ok((None, focus)) => {
                    let focus = focus.or_else(|| second.leaf_ids().first().copied());
                    Ok((Some(*second), focus))
                }
                Ok((Some(f), focus)) => Ok((
                    Some(LiveNode::Split {
                        axis,
                        ratio,
                        first: Box::new(f),
                        second,
                    }),
                    focus,
                )),
                Err(first) => match unsplit_live(*second, empty) {
                    Ok((None, focus)) => {
                        let focus = focus.or_else(|| first.leaf_ids().first().copied());
                        Ok((Some(first), focus))
                    }
                    Ok((Some(s), focus)) => Ok((
                        Some(LiveNode::Split {
                            axis,
                            ratio,
                            first: Box::new(first),
                            second: Box::new(s),
                        }),
                        focus,
                    )),
                    Err(second) => Err(LiveNode::Split {
                        axis,
                        ratio,
                        first: Box::new(first),
                        second: Box::new(second),
                    }),
                },
            }
        }
    }
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
