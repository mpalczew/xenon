//! N-ary split operations on the live pane tree.

use super::*;
use xenon_core::{MAX_NEST_DEPTH, MAX_SPLIT_CHILDREN, SplitTree};

impl SplitTree for LiveNode {
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

impl LiveNode {
    /// Two-child split with equal halves. Splice it into the tree with
    /// [`Self::replace_leaf`], which flattens same-axis nesting.
    pub(crate) fn pair(axis: SplitAxis, first: LiveNode, second: LiveNode) -> LiveNode {
        LiveNode::Split {
            axis,
            children: vec![first, second],
            sizes: vec![0.5, 0.5],
        }
    }

    /// Restore the split invariants everywhere below `self`.
    pub(crate) fn normalize(&mut self) {
        let Self::Split {
            axis,
            children,
            sizes,
        } = self
        else {
            return;
        };
        let axis = *axis;
        for child in children.iter_mut() {
            child.normalize();
        }
        match xenon_core::flatten_split(axis, std::mem::take(children), std::mem::take(sizes)) {
            Ok((c, s)) => {
                *children = c;
                *sizes = s;
            }
            Err(Some(only)) => *self = only,
            // Unreachable: a split always has children.
            Err(None) => {}
        }
    }

    /// (axis alternations above `leaf`, axis and child count of its parent).
    fn split_context(&self, leaf: PaneId) -> Option<(u32, Option<(SplitAxis, usize)>)> {
        match self {
            Self::Leaf(l) => (l.id == leaf).then_some((0, None)),
            Self::Split { axis, children, .. } => children.iter().find_map(|c| {
                let (depth, parent) = c.split_context(leaf)?;
                // The nearest enclosing split is the parent.
                Some((depth + 1, parent.or(Some((*axis, children.len())))))
            }),
        }
    }

    /// Whether splitting `leaf` along `axis` stays within the depth and fan-out limits.
    pub(crate) fn can_split(&self, leaf: PaneId, axis: SplitAxis) -> bool {
        let Some((depth, parent)) = self.split_context(leaf) else {
            return false;
        };
        match parent {
            Some((a, n)) if a == axis => n < MAX_SPLIT_CHILDREN,
            _ => depth < MAX_NEST_DEPTH,
        }
    }

    /// Drag the divider after the child whose last leaf is `key` to `pos`, a
    /// fraction of the split's own extent. Only that child and the next change.
    /// The last leaf is unique per divider.
    pub(crate) fn set_boundary_for_split(&mut self, key: PaneId, pos: f32) -> bool {
        let Self::Split {
            children, sizes, ..
        } = self
        else {
            return false;
        };
        if let Some(i) = children
            .iter()
            .position(|c| c.leaf_ids().last() == Some(&key))
        {
            return xenon_core::move_boundary(sizes, i, pos);
        }
        children
            .iter_mut()
            .any(|c| c.set_boundary_for_split(key, pos))
    }

    /// Replace leaf `id`; same-axis splits are spliced into their parent.
    pub(crate) fn replace_leaf(&mut self, id: PaneId, replacement: LiveNode) -> bool {
        let found = self.replace_leaf_inner(id, replacement);
        if found {
            self.normalize();
        }
        found
    }

    fn replace_leaf_inner(&mut self, id: PaneId, replacement: LiveNode) -> bool {
        match self {
            Self::Leaf(l) if l.id == id => {
                *self = replacement;
                true
            }
            Self::Leaf(_) => false,
            Self::Split { children, .. } => {
                for c in children {
                    if c.replace_leaf_inner(id, replacement.clone()) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Remove leaf `empty` from its split, giving its slot to the previous
    /// neighbour (else the next). Returns the neighbour's first leaf, or `None`
    /// when `empty` is not under a split here.
    pub(super) fn remove_leaf(&mut self, empty: PaneId) -> Option<PaneId> {
        let Self::Split {
            children, sizes, ..
        } = self
        else {
            return None;
        };
        if let Some(i) = children
            .iter()
            .position(|c| matches!(c, Self::Leaf(l) if l.id == empty))
        {
            children.remove(i);
            let slot = sizes.remove(i);
            let n = i.saturating_sub(1);
            sizes[n] += slot;
            return children[n].leaf_ids().first().copied();
        }
        children.iter_mut().find_map(|c| c.remove_leaf(empty))
    }

    /// A leaf next to `id` in its parent split (the next one, else the previous).
    pub(crate) fn sibling_of(&self, id: PaneId) -> Option<PaneId> {
        let Self::Split { children, .. } = self else {
            return None;
        };
        if let Some(i) = children
            .iter()
            .position(|c| matches!(c, Self::Leaf(l) if l.id == id))
        {
            let n = if i + 1 < children.len() { i + 1 } else { i - 1 };
            return children[n].leaf_ids().first().copied();
        }
        children.iter().find_map(|c| c.sibling_of(id))
    }
}
