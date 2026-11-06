//! Live pane tree: the tree type and its basic lookups. Generic over the tab
//! payload, so each frontend stores its own views in the leaves and shares the
//! same behaviour. Split rules, tab lookups, and the moves on a workspace's
//! content live in the sibling `pane_*` modules.

use crate::content::{PaneId, SplitAxis, TabId};
use crate::split::SplitTree;

/// A tab payload that has a stable id.
pub trait PaneTab {
    fn tab_id(&self) -> TabId;
}

/// One leaf: a tab stack with an active tab.
#[derive(Clone)]
pub struct PaneLeaf<T> {
    pub id: PaneId,
    pub tabs: Vec<T>,
    pub active: usize,
    /// Keep this leaf visible when it has no tabs.
    pub parked: bool,
}

impl<T> PaneLeaf<T> {
    pub fn active_tab(&self) -> Option<&T> {
        self.tabs.get(self.active)
    }
}

/// N-ary pane tree. `sizes` are fractions summing to 1 and no child is a
/// split on its parent's axis (see [`flatten_split`]).
#[derive(Clone)]
pub enum PaneTree<T> {
    Leaf(PaneLeaf<T>),
    Split {
        axis: SplitAxis,
        children: Vec<PaneTree<T>>,
        sizes: Vec<f32>,
    },
}

impl<T> SplitTree for PaneTree<T> {
    fn into_axis_parts(self, axis: SplitAxis) -> Result<(Vec<Self>, Vec<f32>), Self> {
        match self {
            Self::Split {
                axis: a,
                children,
                sizes,
            } if a == axis => Ok((children, sizes)),
            other => Err(other),
        }
    }
}

impl<T> PaneTree<T> {
    pub fn for_each_leaf(&self, f: &mut dyn FnMut(&PaneLeaf<T>)) {
        match self {
            Self::Leaf(leaf) => f(leaf),
            Self::Split { children, .. } => {
                for c in children {
                    c.for_each_leaf(f);
                }
            }
        }
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.for_each_leaf(&mut |leaf| out.push(leaf.id));
        out
    }

    pub fn find_leaf(&self, id: PaneId) -> Option<&PaneLeaf<T>> {
        match self {
            Self::Leaf(leaf) if leaf.id == id => Some(leaf),
            Self::Leaf(_) => None,
            Self::Split { children, .. } => children.iter().find_map(|c| c.find_leaf(id)),
        }
    }

    pub fn find_leaf_mut(&mut self, id: PaneId) -> Option<&mut PaneLeaf<T>> {
        match self {
            Self::Leaf(l) if l.id == id => Some(l),
            Self::Leaf(_) => None,
            Self::Split { children, .. } => children.iter_mut().find_map(|c| c.find_leaf_mut(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane_content::PaneContent;

    #[derive(Clone)]
    struct Tab(u64);

    impl PaneTab for Tab {
        fn tab_id(&self) -> TabId {
            TabId(self.0)
        }
    }

    type Node = PaneTree<Tab>;

    fn leaf(id: u64) -> Node {
        PaneTree::Leaf(PaneLeaf {
            id: PaneId(id),
            tabs: Vec::new(),
            active: 0,
            parked: false,
        })
    }

    fn row(ids: &[u64]) -> Node {
        let n = ids.len();
        PaneTree::Split {
            axis: SplitAxis::Horizontal,
            children: ids.iter().map(|i| leaf(*i)).collect(),
            sizes: vec![1.0 / n as f32; n],
        }
    }

    fn sizes(node: &Node) -> Vec<f32> {
        match node {
            PaneTree::Split { sizes, .. } => sizes.clone(),
            PaneTree::Leaf(_) => Vec::new(),
        }
    }

    fn add_beside(tree: &mut Node, target: u64, new: u64) {
        let pair = PaneTree::pair(SplitAxis::Horizontal, leaf(target), leaf(new));
        assert!(tree.replace_leaf(PaneId(target), pair));
    }

    #[test]
    fn same_axis_insert_flattens_and_halves_slot() {
        let mut tree = PaneTree::pair(SplitAxis::Horizontal, leaf(1), leaf(2));
        add_beside(&mut tree, 2, 3);
        add_beside(&mut tree, 3, 4);
        let PaneTree::Split { children, .. } = &tree else {
            panic!("split");
        };
        assert_eq!(children.len(), 4);
        assert!(children.iter().all(|c| matches!(c, PaneTree::Leaf(_))));
        assert_eq!(sizes(&tree), vec![0.5, 0.25, 0.125, 0.125]);
    }

    #[test]
    fn drag_changes_only_adjacent_sizes() {
        let mut tree = row(&[1, 2, 3, 4]);
        assert!(tree.set_boundary_for_split(PaneId(3), 0.6));
        let s = sizes(&tree);
        assert!((s[0] - 0.25).abs() < 1e-6 && (s[1] - 0.25).abs() < 1e-6);
        assert!((s[2] - 0.1).abs() < 1e-6 && (s[3] - 0.4).abs() < 1e-6);
        assert!(!tree.set_boundary_for_split(PaneId(4), 0.5));
    }

    #[test]
    fn closing_a_pane_collapses_single_child_split() {
        let mut content = PaneContent {
            root: Some(row(&[1, 2])),
            focused: Some(PaneId(1)),
        };
        content.unsplit_empty(PaneId(1));
        assert!(matches!(content.root, Some(PaneTree::Leaf(ref l)) if l.id == PaneId(2)));
        assert_eq!(content.focused, Some(PaneId(2)));

        let mut content = PaneContent {
            root: Some(row(&[1, 2, 3])),
            focused: None,
        };
        content.unsplit_empty(PaneId(2));
        let root = content.root.unwrap();
        assert_eq!(root.leaf_ids(), vec![PaneId(1), PaneId(3)]);
        assert!((sizes(&root)[0] - 2.0 / 3.0).abs() < 1e-5);
    }
}
