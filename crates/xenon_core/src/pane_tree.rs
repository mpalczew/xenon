use crate::content::{PaneId, SplitAxis, TabId};
use crate::split::SplitTree;

pub trait PaneTab {
    fn tab_id(&self) -> TabId;
}

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

    pub fn for_each_leaf_mut(&mut self, f: &mut dyn FnMut(&mut PaneLeaf<T>)) {
        match self {
            Self::Leaf(leaf) => f(leaf),
            Self::Split { children, .. } => {
                for c in children {
                    c.for_each_leaf_mut(f);
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
