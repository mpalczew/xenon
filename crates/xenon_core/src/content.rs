//! Content layout: N-ary pane tree of mixed tab stacks (terminals + editors).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::session::Point;
use crate::split::flatten_split;

/// Max axis alternations root→leaf. Same-axis splits are flat, so a long row
/// costs one level.
pub const MAX_NEST_DEPTH: u32 = 3;
/// Max children of one split (each keeps at least `MIN_SIZE`).
pub const MAX_SPLIT_CHILDREN: usize = 8;
/// Smallest fraction of a split any child keeps.
pub const MIN_SIZE: f32 = 0.1;

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

/// Saved pane tree. Splits are N-ary: `sizes` are fractions summing to 1 and no
/// child is a split on its parent's axis. Old binary `first/second/ratio`
/// layouts still load (see `PaneNodeRepr`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", from = "PaneNodeRepr")]
pub enum PaneNode {
    Leaf(LeafPane),
    Split {
        axis: SplitAxis,
        children: Vec<PaneNode>,
        sizes: Vec<f32>,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PaneNodeRepr {
    Leaf(LeafPane),
    Split(SplitRepr),
}

#[derive(Deserialize)]
struct SplitRepr {
    axis: SplitAxis,
    #[serde(default)]
    children: Vec<PaneNode>,
    #[serde(default)]
    sizes: Vec<f32>,
    // Legacy binary shape.
    ratio: Option<f32>,
    first: Option<Box<PaneNode>>,
    second: Option<Box<PaneNode>>,
}

impl From<PaneNodeRepr> for PaneNode {
    fn from(repr: PaneNodeRepr) -> Self {
        match repr {
            PaneNodeRepr::Leaf(leaf) => Self::Leaf(leaf),
            PaneNodeRepr::Split(mut s) => {
                if s.children.is_empty()
                    && let (Some(first), Some(second)) = (s.first, s.second)
                {
                    let ratio = s.ratio.filter(|r| r.is_finite()).unwrap_or(0.5);
                    s.children = vec![*first, *second];
                    s.sizes = vec![ratio, 1.0 - ratio];
                }
                Self::split(s.axis, s.children, s.sizes)
            }
        }
    }
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
            // Empty split: an empty parked leaf keeps the type total.
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
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        self.root
            .as_ref()
            .map(PaneNode::leaf_ids)
            .unwrap_or_default()
    }

    /// Repair focused pointer after load (split sizes are fixed on parse).
    pub fn normalize(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split::move_boundary;

    fn leaf(id: u64) -> serde_json::Value {
        serde_json::json!({"type":"leaf","id":id,"tabs":[]})
    }

    fn old_split(
        axis: &str,
        ratio: f32,
        first: serde_json::Value,
        second: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({"type":"split","axis":axis,"ratio":ratio,"first":first,"second":second})
    }

    #[test]
    fn old_binary_layout_loads_flat() {
        let json = old_split(
            "horizontal",
            0.5,
            old_split(
                "horizontal",
                0.5,
                old_split("horizontal", 0.5, leaf(1), leaf(2)),
                leaf(3),
            ),
            old_split("vertical", 0.4, leaf(4), leaf(5)),
        );
        let node: PaneNode = serde_json::from_value(json).unwrap();
        let PaneNode::Split {
            axis,
            children,
            sizes,
        } = &node
        else {
            panic!("split");
        };
        assert_eq!(*axis, SplitAxis::Horizontal);
        assert_eq!(children.len(), 4);
        assert_eq!(node.leaf_ids().len(), 5);
        assert!((sizes[0] - 0.125).abs() < 1e-6);
        assert!((sizes.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert!(matches!(
            &children[3],
            PaneNode::Split {
                axis: SplitAxis::Vertical,
                ..
            }
        ));
        // Always writes the new shape.
        let out = serde_json::to_value(&node).unwrap();
        assert!(out.get("first").is_none() && out.get("children").is_some());
    }

    #[test]
    fn move_boundary_changes_only_neighbours() {
        let mut sizes = vec![0.25; 4];
        assert!(move_boundary(&mut sizes, 1, 0.4));
        assert!((sizes[0] - 0.25).abs() < 1e-6 && (sizes[3] - 0.25).abs() < 1e-6);
        assert!((sizes[1] - 0.15).abs() < 1e-6 && (sizes[2] - 0.35).abs() < 1e-6);
        assert!(!move_boundary(&mut sizes, 3, 0.5));
    }

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
