use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::TermId;
use crate::session::Point;
use crate::split::flatten_split;

pub(crate) const MAX_NEST_DEPTH: u32 = 3;
pub(crate) const MAX_SPLIT_CHILDREN: usize = 8;
pub(crate) const MIN_SIZE: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TabId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl DropEdge {
    pub fn axis(self) -> SplitAxis {
        match self {
            Self::Left | Self::Right => SplitAxis::Horizontal,
            Self::Top | Self::Bottom => SplitAxis::Vertical,
        }
    }

    pub(crate) fn tab_in_first(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabState {
    Terminal {
        id: TabId,
        #[serde(default = "default_cwd")]
        cwd: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        term: Option<TermId>,
    },
    Editor {
        id: TabId,
        path: PathBuf,
        #[serde(default)]
        cursor: Point,
        #[serde(default)]
        scroll_top: u32,
    },
    Diff {
        id: TabId,
        path: String,
    },
}

fn default_cwd() -> PathBuf {
    PathBuf::from(".")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeafPane {
    pub id: PaneId,
    pub tabs: Vec<TabState>,
    #[serde(default)]
    pub active: usize,
    #[serde(default)]
    pub parked: bool,
}

/// Saved pane tree. Splits are N-ary: `sizes` are fractions summing to 1 and no
/// child is a split on its parent's axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneNode {
    Leaf(LeafPane),
    Split {
        axis: SplitAxis,
        children: Vec<PaneNode>,
        sizes: Vec<f32>,
    },
}

impl PaneNode {
    /// Build a split, enforcing the invariants (may return a single child).
    pub fn split(axis: SplitAxis, children: Vec<PaneNode>, sizes: Vec<f32>) -> PaneNode {
        match flatten_split(axis, children, sizes) {
            Ok((children, sizes)) => PaneNode::Split {
                axis,
                children,
                sizes,
            },
            Err(Some(only)) => only,
            Err(None) => PaneNode::Leaf(LeafPane {
                id: PaneId(1),
                tabs: Vec::new(),
                active: 0,
                parked: true,
            }),
        }
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.collect_leaf_ids(&mut out);
        out
    }

    fn collect_leaf_ids(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(leaf) => out.push(leaf.id),
            Self::Split { children, .. } => {
                for c in children {
                    c.collect_leaf_ids(out);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentLayout {
    #[serde(default)]
    pub root: Option<PaneNode>,
    #[serde(default)]
    pub focused: Option<PaneId>,
}

impl ContentLayout {
    pub(crate) fn normalize(&mut self) {
        if let Some(root) = &mut self.root {
            let leaves = root.leaf_ids();
            if leaves.is_empty() {
                self.root = None;
                self.focused = None;
                return;
            }
            if self.focused.is_none_or(|f| !leaves.contains(&f)) {
                self.focused = Some(leaves[0]);
            }
        } else {
            self.focused = None;
        }
    }
}
