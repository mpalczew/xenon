//! Content layout: binary pane tree of mixed tab stacks (terminals + editors).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::session::Point;

/// Max split depth root→leaf (at most 4 leaves on a path of length 3).
pub const MAX_NEST_DEPTH: u32 = 3;
/// Default first-child fraction when splitting.
pub const DEFAULT_SPLIT_RATIO: f32 = 0.5;
const RATIO_MIN: f32 = 0.15;
const RATIO_MAX: f32 = 0.85;

/// Stable id for a leaf pane within a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneId(pub u64);

/// Stable id for a tab slot within a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TabId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    /// Side by side: first = left, second = right.
    Horizontal,
    /// Stacked: first = top, second = bottom.
    Vertical,
}

/// Edge of a leaf used for tab drop → split.
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

    /// True when the dropped tab should land in the *first* child of the split.
    pub fn tab_in_first(self) -> bool {
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
        remote_session: Option<String>,
    },
    Editor {
        id: TabId,
        path: PathBuf,
        #[serde(default)]
        cursor: Point,
        #[serde(default)]
        scroll_top: u32,
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
    /// Keep this leaf visible when it has no tabs.
    #[serde(default)]
    pub parked: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneNode {
    Leaf(LeafPane),
    Split {
        axis: SplitAxis,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    pub fn leaf_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.collect_leaf_ids(&mut out);
        out
    }

    fn collect_leaf_ids(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(leaf) => out.push(leaf.id),
            Self::Split { first, second, .. } => {
                first.collect_leaf_ids(out);
                second.collect_leaf_ids(out);
            }
        }
    }

    fn clamp_ratios(&mut self) {
        match self {
            Self::Leaf(_) => {}
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                *ratio = clamp_ratio(*ratio);
                first.clamp_ratios();
                second.clamp_ratios();
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
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        self.root
            .as_ref()
            .map(PaneNode::leaf_ids)
            .unwrap_or_default()
    }

    /// Repair focused pointer and clamp ratios after load.
    pub fn normalize(&mut self) {
        if let Some(root) = &mut self.root {
            root.clamp_ratios();
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

fn clamp_ratio(ratio: f32) -> f32 {
    if !ratio.is_finite() {
        return DEFAULT_SPLIT_RATIO;
    }
    ratio.clamp(RATIO_MIN, RATIO_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parked_empty_leaf_round_trips() {
        let content = ContentLayout {
            root: Some(PaneNode::Leaf(LeafPane {
                id: PaneId(1),
                tabs: Vec::new(),
                active: 0,
                parked: true,
            })),
            focused: Some(PaneId(1)),
        };
        let json = serde_json::to_string(&content).unwrap();
        let parsed: ContentLayout = serde_json::from_str(&json).unwrap();
        let PaneNode::Leaf(leaf) = parsed.root.unwrap() else {
            panic!("expected leaf");
        };
        assert!(leaf.parked);
        assert!(leaf.tabs.is_empty());
    }
}
