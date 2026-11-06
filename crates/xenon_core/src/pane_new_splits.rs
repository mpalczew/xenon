//! Splitting a pane in a workspace's pane content, and parking empty panes.

use crate::content::{DropEdge, PaneId, SplitAxis};
use crate::pane_content::{PaneContent, ParkOutcome};
use crate::pane_tree::{PaneLeaf, PaneTree};

impl<T> PaneContent<T> {
    /// Take the active tab of `pane` for a split. Returns it with the tabs
    /// that remain (possibly none); the pane is left empty either way.
    /// `None` when the pane is missing or has no tabs.
    pub fn take_active_for_split(&mut self, pane: PaneId) -> Option<(T, Vec<T>)> {
        let leaf = self.root.as_mut()?.find_leaf_mut(pane)?;
        if leaf.tabs.is_empty() {
            return None;
        }
        let idx = leaf.active.min(leaf.tabs.len() - 1);
        let moved = leaf.tabs.remove(idx);
        let remaining = std::mem::take(&mut leaf.tabs);
        Some((moved, remaining))
    }

    /// Fresh leaf holding `tab`.
    fn leaf_with(id: PaneId, tabs: Vec<T>) -> PaneTree<T> {
        PaneTree::Leaf(PaneLeaf {
            id,
            tabs,
            active: 0,
            parked: false,
        })
    }

    /// Finish a split of `src_pane`: it keeps `source_tabs`, a new pane beside
    /// it (right or below per `axis`) holds `moved` and takes focus.
    pub fn finish_split(
        &mut self,
        src_pane: PaneId,
        axis: SplitAxis,
        source_tabs: Vec<T>,
        moved: T,
    ) -> Option<PaneId>
    where
        T: Clone,
    {
        let new_pane = self.next_pane_id();
        let replacement = PaneTree::pair(
            axis,
            Self::leaf_with(src_pane, source_tabs),
            Self::leaf_with(new_pane, vec![moved]),
        );
        if let Some(root) = self.root.as_mut()
            && !root.replace_leaf(src_pane, replacement)
        {
            return None;
        }
        self.focused = Some(new_pane);
        Some(new_pane)
    }

    /// Split `target` on `edge` with `tab` in the new pane, which takes focus.
    /// `None` when `target` is gone.
    pub fn finish_edge_drop(&mut self, target: PaneId, edge: DropEdge, tab: T) -> Option<PaneId>
    where
        T: Clone,
    {
        let target_leaf = self.root.as_ref()?.find_leaf(target)?.clone();
        let new_pane = self.next_pane_id();
        let new_leaf = Self::leaf_with(new_pane, vec![tab]);
        let target_leaf = PaneTree::Leaf(target_leaf);
        let (first, second) = if edge.tab_in_first() {
            (new_leaf, target_leaf)
        } else {
            (target_leaf, new_leaf)
        };
        let split = PaneTree::pair(edge.axis(), first, second);
        if let Some(root) = self.root.as_mut() {
            root.replace_leaf(target, split);
        }
        self.focused = Some(new_pane);
        Some(new_pane)
    }

    /// Add a deliberately empty pane beside the focused one (or create the
    /// first two panes when there are none).
    pub fn park_empty(&mut self, axis: SplitAxis) -> ParkOutcome
    where
        T: Clone,
    {
        if self.root.is_none() {
            let first = self.next_pane_id();
            let second = PaneId(first.0.saturating_add(1));
            let parked = PaneTree::Leaf(PaneLeaf {
                id: second,
                tabs: Vec::new(),
                active: 0,
                parked: true,
            });
            self.root = Some(PaneTree::pair(
                axis,
                Self::leaf_with(first, Vec::new()),
                parked,
            ));
            self.focused = Some(first);
            return ParkOutcome::Created;
        }
        let Some(source) = self.focused else {
            return ParkOutcome::Unchanged;
        };
        let Some((can_split, mut source_leaf)) = self.root.as_ref().and_then(|root| {
            Some((
                root.can_split(source, axis),
                root.find_leaf(source)?.clone(),
            ))
        }) else {
            return ParkOutcome::Unchanged;
        };
        if !can_split {
            return ParkOutcome::NoRoom;
        }
        source_leaf.parked = false;
        let new_pane = self.next_pane_id();
        let replacement = PaneTree::pair(
            axis,
            PaneTree::Leaf(source_leaf),
            PaneTree::Leaf(PaneLeaf {
                id: new_pane,
                tabs: Vec::new(),
                active: 0,
                parked: true,
            }),
        );
        if self
            .root
            .as_mut()
            .is_some_and(|root| root.replace_leaf(source, replacement))
        {
            self.focused = Some(source);
            ParkOutcome::Added(source)
        } else {
            ParkOutcome::Unchanged
        }
    }
}
