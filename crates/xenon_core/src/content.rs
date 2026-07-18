//! Content layout: binary pane tree of mixed tab stacks (terminals + editors).

#![allow(dead_code)] // pure model; UI uses a subset of ops
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::session::{OpenEditor, Point, TerminalState};

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

#[allow(dead_code)]
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

impl TabState {
    pub(crate) fn id(&self) -> TabId {
        match self {
            Self::Terminal { id, .. } | Self::Editor { id, .. } => *id,
        }
    }

    pub(crate) fn is_editor_path(&self, path: &std::path::Path) -> bool {
        matches!(self, Self::Editor { path: p, .. } if p == path)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeafPane {
    pub id: PaneId,
    pub tabs: Vec<TabState>,
    #[serde(default)]
    pub active: usize,
}

impl LeafPane {
    pub(crate) fn new(id: PaneId, tabs: Vec<TabState>, active: usize) -> Self {
        let active = if tabs.is_empty() {
            0
        } else {
            active.min(tabs.len() - 1)
        };
        Self { id, tabs, active }
    }

    pub(crate) fn active_tab(&self) -> Option<&TabState> {
        self.tabs.get(self.active)
    }
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

    pub(crate) fn find_leaf(&self, id: PaneId) -> Option<&LeafPane> {
        match self {
            Self::Leaf(leaf) if leaf.id == id => Some(leaf),
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => {
                first.find_leaf(id).or_else(|| second.find_leaf(id))
            }
        }
    }

    pub(crate) fn find_leaf_mut(&mut self, id: PaneId) -> Option<&mut LeafPane> {
        match self {
            Self::Leaf(leaf) if leaf.id == id => Some(leaf),
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => {
                first.find_leaf_mut(id).or_else(|| second.find_leaf_mut(id))
            }
        }
    }

    pub(crate) fn find_tab(&self, tab: TabId) -> Option<(PaneId, usize)> {
        match self {
            Self::Leaf(leaf) => leaf
                .tabs
                .iter()
                .position(|t| t.id() == tab)
                .map(|i| (leaf.id, i)),
            Self::Split { first, second, .. } => {
                first.find_tab(tab).or_else(|| second.find_tab(tab))
            }
        }
    }

    pub(crate) fn find_editor_path(&self, path: &std::path::Path) -> Option<(PaneId, usize)> {
        match self {
            Self::Leaf(leaf) => leaf
                .tabs
                .iter()
                .position(|t| t.is_editor_path(path))
                .map(|i| (leaf.id, i)),
            Self::Split { first, second, .. } => first
                .find_editor_path(path)
                .or_else(|| second.find_editor_path(path)),
        }
    }

    /// Depth of splits on the path to `leaf` (0 = root is that leaf).
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

    fn next_ids(&self) -> (u64, u64) {
        let mut max_pane = 0u64;
        let mut max_tab = 0u64;
        self.walk_ids(&mut max_pane, &mut max_tab);
        (max_pane + 1, max_tab + 1)
    }

    fn walk_ids(&self, max_pane: &mut u64, max_tab: &mut u64) {
        match self {
            Self::Leaf(leaf) => {
                *max_pane = (*max_pane).max(leaf.id.0);
                for t in &leaf.tabs {
                    *max_tab = (*max_tab).max(t.id().0);
                }
            }
            Self::Split { first, second, .. } => {
                first.walk_ids(max_pane, max_tab);
                second.walk_ids(max_pane, max_tab);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    NoFocus,
    AtNestCap,
    UnknownId,
    EmptyTree,
}

#[allow(dead_code)]
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

    pub(crate) fn focused_leaf(&self) -> Option<&LeafPane> {
        let id = self.focused?;
        self.root.as_ref()?.find_leaf(id)
    }

    pub(crate) fn focused_leaf_mut(&mut self) -> Option<&mut LeafPane> {
        let id = self.focused?;
        self.root.as_mut()?.find_leaf_mut(id)
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

    fn alloc_ids(&self) -> (u64, u64) {
        match &self.root {
            Some(root) => root.next_ids(),
            None => (1, 1),
        }
    }

    /// Bootstrap or append a terminal tab in the focused leaf.
    pub(crate) fn open_terminal(&mut self, cwd: PathBuf) -> TabId {
        let (next_pane, next_tab) = self.alloc_ids();
        let tab_id = TabId(next_tab);
        let tab = TabState::Terminal { id: tab_id, cwd };
        if self.root.is_none() {
            let pane_id = PaneId(next_pane);
            self.root = Some(PaneNode::Leaf(LeafPane::new(pane_id, vec![tab], 0)));
            self.focused = Some(pane_id);
            return tab_id;
        }
        let focused = self.focused.or_else(|| self.leaf_ids().first().copied());
        let Some(fid) = focused else {
            let pane_id = PaneId(next_pane);
            self.root = Some(PaneNode::Leaf(LeafPane::new(pane_id, vec![tab], 0)));
            self.focused = Some(pane_id);
            return tab_id;
        };
        self.focused = Some(fid);
        if let Some(leaf) = self.root.as_mut().and_then(|r| r.find_leaf_mut(fid)) {
            leaf.tabs.push(tab);
            leaf.active = leaf.tabs.len() - 1;
        }
        tab_id
    }

    /// Open or focus an editor tab. Returns (pane, tab, newly_opened).
    pub(crate) fn open_editor(
        &mut self,
        path: PathBuf,
        cursor: Point,
        scroll_top: u32,
    ) -> (PaneId, TabId, bool) {
        if let Some(root) = &self.root
            && let Some((pane, idx)) = root.find_editor_path(&path)
        {
            if let Some(leaf) = self.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) {
                leaf.active = idx;
            }
            self.focused = Some(pane);
            let tab_id = self
                .root
                .as_ref()
                .and_then(|r| r.find_leaf(pane))
                .and_then(|l| l.tabs.get(idx))
                .map(|t| t.id())
                .unwrap_or(TabId(0));
            return (pane, tab_id, false);
        }

        let (next_pane, next_tab) = self.alloc_ids();
        let tab_id = TabId(next_tab);
        let tab = TabState::Editor {
            id: tab_id,
            path,
            cursor,
            scroll_top,
        };
        if self.root.is_none() {
            let pane_id = PaneId(next_pane);
            self.root = Some(PaneNode::Leaf(LeafPane::new(pane_id, vec![tab], 0)));
            self.focused = Some(pane_id);
            return (pane_id, tab_id, true);
        }
        let fid = self
            .focused
            .or_else(|| self.leaf_ids().first().copied())
            .unwrap_or(PaneId(next_pane));
        if let Some(leaf) = self.root.as_mut().and_then(|r| r.find_leaf_mut(fid)) {
            leaf.tabs.push(tab);
            leaf.active = leaf.tabs.len() - 1;
            self.focused = Some(fid);
            return (fid, tab_id, true);
        }
        let pane_id = PaneId(next_pane);
        self.root = Some(PaneNode::Leaf(LeafPane::new(pane_id, vec![tab], 0)));
        self.focused = Some(pane_id);
        (pane_id, tab_id, true)
    }

    /// Split focused leaf; move active tab to new sibling.
    /// If source would be empty, insert a fresh terminal there.
    pub(crate) fn split(
        &mut self,
        axis: SplitAxis,
    ) -> Result<(PaneId, Option<TabId>), LayoutError> {
        let focused = self.focused.ok_or(LayoutError::NoFocus)?;
        let root = self.root.as_ref().ok_or(LayoutError::EmptyTree)?;
        let depth = root.nest_depth(focused).ok_or(LayoutError::UnknownId)?;
        if depth >= MAX_NEST_DEPTH {
            return Err(LayoutError::AtNestCap);
        }
        let leaf = root.find_leaf(focused).ok_or(LayoutError::UnknownId)?;
        if leaf.tabs.is_empty() {
            return Err(LayoutError::UnknownId);
        }
        let active_idx = leaf.active.min(leaf.tabs.len() - 1);
        let moved = leaf.tabs[active_idx].clone();
        let remaining: Vec<TabState> = leaf
            .tabs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != active_idx)
            .map(|(_, t)| t.clone())
            .collect();

        let (next_pane, next_tab) = self.alloc_ids();
        let new_pane_id = PaneId(next_pane);
        let spawn_term_id = if remaining.is_empty() {
            Some(TabId(next_tab))
        } else {
            None
        };

        let source_tabs = if let Some(tid) = spawn_term_id {
            vec![TabState::Terminal {
                id: tid,
                cwd: PathBuf::from("."),
            }]
        } else {
            remaining
        };
        let source_leaf = LeafPane::new(focused, source_tabs, 0);
        let new_leaf = LeafPane::new(new_pane_id, vec![moved], 0);

        let replacement = PaneNode::Split {
            axis,
            ratio: DEFAULT_SPLIT_RATIO,
            first: Box::new(PaneNode::Leaf(source_leaf)),
            second: Box::new(PaneNode::Leaf(new_leaf)),
        };
        replace_leaf(&mut self.root, focused, replacement)?;
        self.focused = Some(new_pane_id);
        Ok((new_pane_id, spawn_term_id))
    }

    /// Move tab to another leaf (center drop). Unsplits source if emptied.
    pub(crate) fn move_tab(&mut self, tab: TabId, dest: PaneId) -> Result<(), LayoutError> {
        self.move_tab_impl(tab, dest)
    }

    fn move_tab_impl(&mut self, tab: TabId, dest: PaneId) -> Result<(), LayoutError> {
        let (src_pane, src_idx) = self
            .root
            .as_ref()
            .and_then(|r| r.find_tab(tab))
            .ok_or(LayoutError::UnknownId)?;
        if src_pane == dest {
            if let Some(leaf) = self.root.as_mut().and_then(|r| r.find_leaf_mut(src_pane)) {
                leaf.active = src_idx;
            }
            self.focused = Some(src_pane);
            return Ok(());
        }
        if self.root.as_ref().and_then(|r| r.find_leaf(dest)).is_none() {
            return Err(LayoutError::UnknownId);
        }

        let tab_state = {
            let leaf = self
                .root
                .as_mut()
                .and_then(|r| r.find_leaf_mut(src_pane))
                .ok_or(LayoutError::UnknownId)?;
            let t = leaf.tabs.remove(src_idx);
            if leaf.tabs.is_empty() {
                // leave empty; unsplit below
            } else {
                fix_active(&mut leaf.active, src_idx, leaf.tabs.len());
            }
            t
        };

        {
            let leaf = self
                .root
                .as_mut()
                .and_then(|r| r.find_leaf_mut(dest))
                .ok_or(LayoutError::UnknownId)?;
            leaf.tabs.push(tab_state);
            leaf.active = leaf.tabs.len() - 1;
        }

        if self
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(src_pane))
            .is_some_and(|l| l.tabs.is_empty())
        {
            self.unsplit_empty_leaf(src_pane)?;
        }
        self.focused = Some(dest);
        self.normalize();
        Ok(())
    }

    /// Drop tab on edge of target leaf → split target and place tab.
    pub(crate) fn drop_tab_on_edge(
        &mut self,
        tab: TabId,
        target: PaneId,
        edge: DropEdge,
    ) -> Result<PaneId, LayoutError> {
        let root = self.root.as_ref().ok_or(LayoutError::EmptyTree)?;
        let (src_pane, _) = root.find_tab(tab).ok_or(LayoutError::UnknownId)?;
        let depth = root.nest_depth(target).ok_or(LayoutError::UnknownId)?;
        // If target is same as source and only one tab, splitting is still ok if nest allows.
        if depth >= MAX_NEST_DEPTH {
            // Fall back to move into target if possible
            if src_pane != target {
                self.move_tab_impl(tab, target)?;
            }
            return Err(LayoutError::AtNestCap);
        }

        let tab_state = {
            let leaf = self
                .root
                .as_mut()
                .and_then(|r| r.find_leaf_mut(src_pane))
                .ok_or(LayoutError::UnknownId)?;
            let idx = leaf
                .tabs
                .iter()
                .position(|t| t.id() == tab)
                .ok_or(LayoutError::UnknownId)?;
            let t = leaf.tabs.remove(idx);
            if !leaf.tabs.is_empty() {
                fix_active(&mut leaf.active, idx, leaf.tabs.len());
            }
            t
        };

        // If source emptied and source == target, we cannot split an empty leaf.
        if src_pane == target
            && self
                .root
                .as_ref()
                .and_then(|r| r.find_leaf(target))
                .is_some_and(|l| l.tabs.is_empty())
        {
            // Put tab back and abort
            if let Some(leaf) = self.root.as_mut().and_then(|r| r.find_leaf_mut(target)) {
                leaf.tabs.push(tab_state);
                leaf.active = leaf.tabs.len() - 1;
            }
            return Err(LayoutError::UnknownId);
        }

        if src_pane != target
            && self
                .root
                .as_ref()
                .and_then(|r| r.find_leaf(src_pane))
                .is_some_and(|l| l.tabs.is_empty())
        {
            self.unsplit_empty_leaf(src_pane)?;
            // target id still valid if it wasn't removed
        }

        let (next_pane, _) = self.alloc_ids();
        let new_id = PaneId(next_pane);
        let new_leaf = LeafPane::new(new_id, vec![tab_state], 0);

        let target_leaf = self
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(target))
            .ok_or(LayoutError::UnknownId)?
            .clone();

        let (first, second) = if edge.tab_in_first() {
            (PaneNode::Leaf(new_leaf), PaneNode::Leaf(target_leaf))
        } else {
            (PaneNode::Leaf(target_leaf), PaneNode::Leaf(new_leaf))
        };
        let replacement = PaneNode::Split {
            axis: edge.axis(),
            ratio: DEFAULT_SPLIT_RATIO,
            first: Box::new(first),
            second: Box::new(second),
        };
        replace_leaf(&mut self.root, target, replacement)?;
        self.focused = Some(new_id);
        self.normalize();
        Ok(new_id)
    }

    /// Close a tab; unsplit or empty tree as needed. Does not spawn terminals.
    pub(crate) fn close_tab(&mut self, tab: TabId) -> Result<(), LayoutError> {
        let (pane, idx) = self
            .root
            .as_ref()
            .and_then(|r| r.find_tab(tab))
            .ok_or(LayoutError::UnknownId)?;
        {
            let leaf = self
                .root
                .as_mut()
                .and_then(|r| r.find_leaf_mut(pane))
                .ok_or(LayoutError::UnknownId)?;
            leaf.tabs.remove(idx);
            if !leaf.tabs.is_empty() {
                fix_active(&mut leaf.active, idx, leaf.tabs.len());
                self.focused = Some(pane);
                return Ok(());
            }
        }
        // Leaf empty.
        let leaf_count = self.leaf_ids().len();
        if leaf_count <= 1 {
            self.root = None;
            self.focused = None;
            return Ok(());
        }
        self.unsplit_empty_leaf(pane)?;
        self.normalize();
        Ok(())
    }

    pub(crate) fn set_ratio(
        &mut self,
        first_leaf_in_split: PaneId,
        ratio: f32,
    ) -> Result<(), LayoutError> {
        let ratio = clamp_ratio(ratio);
        set_ratio_for_leaf(&mut self.root, first_leaf_in_split, ratio)
            .then_some(())
            .ok_or(LayoutError::UnknownId)
    }

    pub(crate) fn activate_tab(&mut self, pane: PaneId, index: usize) -> Result<(), LayoutError> {
        let leaf = self
            .root
            .as_mut()
            .and_then(|r| r.find_leaf_mut(pane))
            .ok_or(LayoutError::UnknownId)?;
        if index >= leaf.tabs.len() {
            return Err(LayoutError::UnknownId);
        }
        leaf.active = index;
        self.focused = Some(pane);
        Ok(())
    }

    pub(crate) fn focus_leaf(&mut self, pane: PaneId) -> Result<(), LayoutError> {
        if self.root.as_ref().and_then(|r| r.find_leaf(pane)).is_none() {
            return Err(LayoutError::UnknownId);
        }
        self.focused = Some(pane);
        Ok(())
    }

    fn unsplit_empty_leaf(&mut self, empty: PaneId) -> Result<(), LayoutError> {
        let root = self.root.take().ok_or(LayoutError::EmptyTree)?;
        match unsplit_node(root, empty) {
            Ok((new_root, focus)) => {
                self.root = new_root;
                if let Some(f) = focus {
                    self.focused = Some(f);
                } else if let Some(r) = &self.root {
                    self.focused = r.leaf_ids().first().copied();
                } else {
                    self.focused = None;
                }
                Ok(())
            }
            Err(root) => {
                self.root = Some(root);
                Err(LayoutError::UnknownId)
            }
        }
    }
}

fn clamp_ratio(ratio: f32) -> f32 {
    if !ratio.is_finite() {
        return DEFAULT_SPLIT_RATIO;
    }
    ratio.clamp(RATIO_MIN, RATIO_MAX)
}

fn fix_active(active: &mut usize, removed: usize, len: usize) {
    if *active > removed {
        *active -= 1;
    } else if *active >= len {
        *active = len.saturating_sub(1);
    }
}

fn replace_leaf(
    root: &mut Option<PaneNode>,
    id: PaneId,
    replacement: PaneNode,
) -> Result<(), LayoutError> {
    let node = root.as_mut().ok_or(LayoutError::EmptyTree)?;
    if replace_leaf_in(node, id, replacement) {
        Ok(())
    } else {
        Err(LayoutError::UnknownId)
    }
}

fn replace_leaf_in(node: &mut PaneNode, id: PaneId, replacement: PaneNode) -> bool {
    match node {
        PaneNode::Leaf(leaf) if leaf.id == id => {
            *node = replacement;
            true
        }
        PaneNode::Leaf(_) => false,
        PaneNode::Split { first, second, .. } => {
            replace_leaf_in(first, id, replacement.clone())
                || replace_leaf_in(second, id, replacement)
        }
    }
}

/// Remove empty leaf. Ok = (new subtree or None if whole node gone, focus hint).
/// Err returns the original node unchanged.
fn unsplit_node(
    node: PaneNode,
    empty: PaneId,
) -> Result<(Option<PaneNode>, Option<PaneId>), PaneNode> {
    match node {
        PaneNode::Leaf(leaf) if leaf.id == empty => Ok((None, None)),
        PaneNode::Leaf(leaf) => Err(PaneNode::Leaf(leaf)),
        PaneNode::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            if matches!(first.as_ref(), PaneNode::Leaf(l) if l.id == empty) {
                let focus = second.leaf_ids().first().copied();
                return Ok((Some(*second), focus));
            }
            if matches!(second.as_ref(), PaneNode::Leaf(l) if l.id == empty) {
                let focus = first.leaf_ids().first().copied();
                return Ok((Some(*first), focus));
            }
            match unsplit_node(*first, empty) {
                Ok((None, focus)) => {
                    let focus = focus.or_else(|| second.leaf_ids().first().copied());
                    Ok((Some(*second), focus))
                }
                Ok((Some(f), focus)) => Ok((
                    Some(PaneNode::Split {
                        axis,
                        ratio,
                        first: Box::new(f),
                        second,
                    }),
                    focus,
                )),
                Err(first) => match unsplit_node(*second, empty) {
                    Ok((None, focus)) => {
                        let focus = focus.or_else(|| first.leaf_ids().first().copied());
                        Ok((Some(first), focus))
                    }
                    Ok((Some(s), focus)) => Ok((
                        Some(PaneNode::Split {
                            axis,
                            ratio,
                            first: Box::new(first),
                            second: Box::new(s),
                        }),
                        focus,
                    )),
                    Err(second) => Err(PaneNode::Split {
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

fn set_ratio_for_leaf(root: &mut Option<PaneNode>, leaf: PaneId, ratio: f32) -> bool {
    let Some(node) = root else {
        return false;
    };
    set_ratio_in(node, leaf, ratio)
}

fn set_ratio_in(node: &mut PaneNode, leaf: PaneId, ratio: f32) -> bool {
    match node {
        PaneNode::Leaf(_) => false,
        PaneNode::Split {
            ratio: r,
            first,
            second,
            ..
        } => {
            if first.find_leaf(leaf).is_some() && second.find_leaf(leaf).is_none() {
                // leaf only in first — if first is the leaf itself, this is the split
                if matches!(first.as_ref(), PaneNode::Leaf(l) if l.id == leaf)
                    || first.leaf_ids().contains(&leaf)
                {
                    // Prefer the split whose first child subtree contains leaf and
                    // is the direct parent when leaf is direct child.
                    if matches!(first.as_ref(), PaneNode::Leaf(l) if l.id == leaf) {
                        *r = ratio;
                        return true;
                    }
                }
            }
            if matches!(first.as_ref(), PaneNode::Leaf(l) if l.id == leaf)
                || matches!(second.as_ref(), PaneNode::Leaf(l) if l.id == leaf)
            {
                *r = ratio;
                return true;
            }
            set_ratio_in(first, leaf, ratio) || set_ratio_in(second, leaf, ratio)
        }
    }
}

/// Build content layout from the pre-pane-tree session fields.
/// Inputs from the pre-pane-tree session JSON shape.
pub struct LegacySession {
    pub terminal_visible: bool,
    pub editor_visible: bool,
    pub terminal_width: f32,
    pub editors: Vec<OpenEditor>,
    pub active_editor: Option<usize>,
    pub terminal: TerminalState,
}

pub fn migrate_from_legacy(legacy: LegacySession) -> ContentLayout {
    let LegacySession {
        terminal_visible,
        editor_visible,
        terminal_width,
        editors,
        active_editor,
        terminal,
    } = legacy;
    let mut next_pane = 1u64;
    let mut next_tab = 1u64;
    let mut alloc_pane = || {
        let id = PaneId(next_pane);
        next_pane += 1;
        id
    };
    let mut alloc_tab = || {
        let id = TabId(next_tab);
        next_tab += 1;
        id
    };

    let make_term = |alloc_tab: &mut dyn FnMut() -> TabId| TabState::Terminal {
        id: alloc_tab(),
        cwd: terminal.cwd.clone(),
    };
    let make_editors = |alloc_tab: &mut dyn FnMut() -> TabId| -> Vec<TabState> {
        editors
            .iter()
            .map(|e| TabState::Editor {
                id: alloc_tab(),
                path: e.path.clone(),
                cursor: e.cursor,
                scroll_top: e.scroll_top,
            })
            .collect()
    };

    let has_editors = !editors.is_empty();
    let mut content = if terminal_visible && editor_visible && has_editors {
        let term_id = alloc_pane();
        let ed_id = alloc_pane();
        let ratio = clamp_ratio(terminal_width / (terminal_width + 400.0).max(1.0));
        let active = active_editor.unwrap_or(0);
        ContentLayout {
            root: Some(PaneNode::Split {
                axis: SplitAxis::Horizontal,
                ratio,
                first: Box::new(PaneNode::Leaf(LeafPane::new(
                    term_id,
                    vec![make_term(&mut alloc_tab)],
                    0,
                ))),
                second: Box::new(PaneNode::Leaf(LeafPane::new(
                    ed_id,
                    make_editors(&mut alloc_tab),
                    active,
                ))),
            }),
            focused: Some(term_id),
        }
    } else if has_editors && (editor_visible || !terminal_visible) {
        let ed_id = alloc_pane();
        let active = active_editor.unwrap_or(0);
        ContentLayout {
            root: Some(PaneNode::Leaf(LeafPane::new(
                ed_id,
                make_editors(&mut alloc_tab),
                active,
            ))),
            focused: Some(ed_id),
        }
    } else if terminal_visible || !has_editors {
        let term_id = alloc_pane();
        ContentLayout {
            root: Some(PaneNode::Leaf(LeafPane::new(
                term_id,
                vec![make_term(&mut alloc_tab)],
                0,
            ))),
            focused: Some(term_id),
        }
    } else {
        ContentLayout::default()
    };
    content.normalize();
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single_term() -> ContentLayout {
        let mut c = ContentLayout::default();
        c.open_terminal(PathBuf::from("."));
        c
    }

    #[test]
    fn open_terminal_bootstraps() {
        let c = single_term();
        assert!(!c.is_empty());
        assert_eq!(c.leaf_ids().len(), 1);
        assert!(c.focused.is_some());
    }

    #[test]
    fn open_editor_same_leaf() {
        let mut c = single_term();
        let pane = c.focused.unwrap();
        c.open_editor(PathBuf::from("a.rs"), Point::default(), 0);
        assert_eq!(c.leaf_ids(), vec![pane]);
        assert_eq!(c.focused_leaf().unwrap().tabs.len(), 2);
    }

    #[test]
    fn open_editor_dedupes() {
        let mut c = single_term();
        let (p1, t1, new1) = c.open_editor(PathBuf::from("a.rs"), Point::default(), 0);
        let (p2, t2, new2) = c.open_editor(PathBuf::from("a.rs"), Point::default(), 0);
        assert!(new1);
        assert!(!new2);
        assert_eq!(p1, p2);
        assert_eq!(t1, t2);
        assert_eq!(c.focused_leaf().unwrap().tabs.len(), 2); // term + one editor
    }

    #[test]
    fn split_moves_active_and_spawns_term() {
        let mut c = single_term();
        c.open_editor(PathBuf::from("a.rs"), Point::default(), 0);
        // active is editor
        let (new_pane, spawned) = c.split(SplitAxis::Horizontal).unwrap();
        assert!(spawned.is_none()); // source still has terminal
        assert_eq!(c.leaf_ids().len(), 2);
        assert_eq!(c.focused, Some(new_pane));
        let leaves: Vec<_> = c
            .leaf_ids()
            .into_iter()
            .map(|id| c.root.as_ref().unwrap().find_leaf(id).unwrap())
            .collect();
        assert!(leaves.iter().any(|l| l.tabs.len() == 1));
    }

    #[test]
    fn split_single_tab_spawns_terminal() {
        let mut c = single_term();
        let (_new, spawned) = c.split(SplitAxis::Horizontal).unwrap();
        assert!(spawned.is_some());
        assert_eq!(c.leaf_ids().len(), 2);
    }

    #[test]
    fn close_last_unsplits() {
        let mut c = single_term();
        c.open_editor(PathBuf::from("a.rs"), Point::default(), 0);
        c.split(SplitAxis::Horizontal).unwrap();
        assert_eq!(c.leaf_ids().len(), 2);
        let editor_tab = c
            .root
            .as_ref()
            .unwrap()
            .find_editor_path(std::path::Path::new("a.rs"))
            .map(|(pane, idx)| c.root.as_ref().unwrap().find_leaf(pane).unwrap().tabs[idx].id())
            .unwrap();
        c.close_tab(editor_tab).unwrap();
        assert_eq!(c.leaf_ids().len(), 1);
    }

    #[test]
    fn close_last_overall_empties() {
        let mut c = single_term();
        let tab = c.focused_leaf().unwrap().tabs[0].id();
        c.close_tab(tab).unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn nest_cap() {
        let mut c = single_term();
        c.split(SplitAxis::Horizontal).unwrap();
        c.split(SplitAxis::Vertical).unwrap();
        c.split(SplitAxis::Horizontal).unwrap();
        assert_eq!(c.split(SplitAxis::Vertical), Err(LayoutError::AtNestCap));
    }

    #[test]
    fn migrate_both_visible() {
        let c = migrate_from_legacy(LegacySession {
            terminal_visible: true,
            editor_visible: true,
            terminal_width: 520.0,
            editors: vec![OpenEditor {
                path: PathBuf::from("x.rs"),
                cursor: Point::default(),
                scroll_top: 0,
            }],
            active_editor: Some(0),
            terminal: TerminalState::default(),
        });
        assert_eq!(c.leaf_ids().len(), 2);
    }

    #[test]
    fn content_round_trip_json() {
        let mut c = single_term();
        c.open_editor(PathBuf::from("a.rs"), Point::default(), 0);
        c.split(SplitAxis::Horizontal).unwrap();
        let json = serde_json::to_string(&c).unwrap();
        let parsed: ContentLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.leaf_ids().len(), 2);
    }

    #[test]
    fn move_tab_between_leaves() {
        let mut c = single_term();
        c.open_editor(PathBuf::from("a.rs"), Point::default(), 0);
        c.split(SplitAxis::Horizontal).unwrap();
        let leaves = c.leaf_ids();
        assert_eq!(leaves.len(), 2);
        // Find terminal tab and move to other leaf
        let (term_pane, term_tab) = {
            let root = c.root.as_ref().unwrap();
            let mut found = None;
            for pid in &leaves {
                let leaf = root.find_leaf(*pid).unwrap();
                for t in &leaf.tabs {
                    if matches!(t, TabState::Terminal { .. }) {
                        found = Some((*pid, t.id()));
                    }
                }
            }
            found.unwrap()
        };
        let dest = leaves.into_iter().find(|p| *p != term_pane).unwrap();
        c.move_tab(term_tab, dest).unwrap();
        // One leaf (unsplit) with 2 tabs
        assert_eq!(c.leaf_ids().len(), 1);
        assert_eq!(c.focused_leaf().unwrap().tabs.len(), 2);
    }
}
