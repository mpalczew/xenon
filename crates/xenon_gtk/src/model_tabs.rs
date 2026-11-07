//! Tab and pane commands on the model. Each returns what the view needs to
//! know; the pane-tree rules themselves live in `xenon_core`.

use xenon_core::{
    CloseOutcome, PaneId, PaneLeaf, PaneTree, ParkOutcome, SplitAxis, TabId, TabState, WorkspaceId,
};

use crate::model::{Model, Tab, TabKind};

/// What `split` did.
pub(crate) enum SplitResult {
    /// Split made; a fresh terminal was added when the source pane would
    /// otherwise have been left empty.
    Done,
    NoRoom,
    Nothing,
}

impl Model {
    fn space_content(&mut self, ws: WorkspaceId) -> Option<&mut xenon_core::PaneContent<Tab>> {
        self.spaces.get_mut(&ws).map(|s| &mut s.content)
    }

    /// Add a terminal to the focused pane (creating the first pane if needed).
    pub(crate) fn new_terminal(&mut self, ws: WorkspaceId) -> Option<TabId> {
        let content = self.space_content(ws)?;
        let id = content.next_tab_id();
        if content.root.is_none() {
            let pane = PaneId(1);
            content.root = Some(PaneTree::Leaf(PaneLeaf {
                id: pane,
                tabs: Vec::new(),
                active: 0,
                parked: false,
            }));
            content.focused = Some(pane);
        }
        if content.focused_leaf().is_none() {
            content.focused = content.leaf_ids().first().copied();
        }
        let leaf = content.focused_leaf_mut()?;
        leaf.tabs.push(Tab::terminal(id));
        leaf.active = leaf.tabs.len() - 1;
        leaf.parked = false;
        Some(id)
    }

    /// Add a tab to the focused pane (creating the first pane if needed) and
    /// make it the active one.
    fn push_tab(&mut self, ws: WorkspaceId, make: impl FnOnce(TabId) -> Tab) -> Option<TabId> {
        let content = self.space_content(ws)?;
        let id = content.next_tab_id();
        if content.root.is_none() {
            let pane = PaneId(1);
            content.root = Some(PaneTree::Leaf(PaneLeaf {
                id: pane,
                tabs: Vec::new(),
                active: 0,
                parked: false,
            }));
            content.focused = Some(pane);
        }
        if content.focused_leaf().is_none() {
            content.focused = content.leaf_ids().first().copied();
        }
        let leaf = content.focused_leaf_mut()?;
        leaf.tabs.push(make(id));
        leaf.active = leaf.tabs.len() - 1;
        leaf.parked = false;
        Some(id)
    }

    /// The tab of `ws` that `matches`, if any.
    fn find_tab_where(&mut self, ws: WorkspaceId, matches: impl Fn(&Tab) -> bool) -> Option<TabId> {
        let content = self.space_content(ws)?;
        let mut found = None;
        if let Some(root) = content.root.as_ref() {
            root.for_each_leaf(&mut |leaf| {
                for tab in &leaf.tabs {
                    if matches(tab) {
                        found = Some(tab.id);
                    }
                }
            });
        }
        found
    }

    /// Show `path` in an editor tab: the one already open for it, or a new tab
    /// in the focused pane. Returns the tab.
    pub(crate) fn open_editor(&mut self, ws: WorkspaceId, path: &std::path::Path) -> Option<TabId> {
        let existing = self.find_tab_where(
            ws,
            |tab| matches!(&tab.kind, TabKind::Kept(TabState::Editor { path: p, .. }) if p == path),
        );
        if let Some(id) = existing {
            self.select_tab(ws, id);
            return Some(id);
        }
        let path = path.to_path_buf();
        self.push_tab(ws, |id| Tab {
            id,
            kind: TabKind::Kept(TabState::Editor {
                id,
                path,
                cursor: Default::default(),
                scroll_top: 0,
            }),
            title: String::new(),
            exited: None,
            modified: false,
            phone_device: None,
        })
    }

    /// Show the diff of `path` (relative to the workspace): the tab already
    /// open for it, or a new one in the focused pane. The bool says whether
    /// the tab is new.
    pub(crate) fn open_diff(&mut self, ws: WorkspaceId, path: &str) -> Option<(TabId, bool)> {
        let existing = self.find_tab_where(
            ws,
            |tab| matches!(&tab.kind, TabKind::Diff { path: p } if p == path),
        );
        if let Some(id) = existing {
            self.select_tab(ws, id);
            return Some((id, false));
        }
        let path = path.to_string();
        let id = self.push_tab(ws, |id| Tab {
            id,
            kind: TabKind::Diff { path },
            title: String::new(),
            exited: None,
            modified: false,
            phone_device: None,
        })?;
        Some((id, true))
    }

    /// Remove an empty pane that has company (the card's "Remove empty pane").
    pub(crate) fn remove_empty_pane(&mut self, ws: WorkspaceId, pane: PaneId) -> bool {
        self.space_content(ws)
            .is_some_and(|content| content.remove_empty_leaf(pane))
    }

    /// Split the focused pane: its active tab moves to the new pane; a pane
    /// left empty gets a fresh terminal.
    pub(crate) fn split(&mut self, ws: WorkspaceId, axis: SplitAxis) -> SplitResult {
        let Some(content) = self.space_content(ws) else {
            return SplitResult::Nothing;
        };
        let Some(src) = content.focused else {
            return SplitResult::Nothing;
        };
        if !content
            .root
            .as_ref()
            .is_some_and(|r| r.can_split(src, axis))
        {
            return SplitResult::NoRoom;
        }
        let fresh = content.next_tab_id();
        let Some((moved, mut remaining)) = content.take_active_for_split(src) else {
            return SplitResult::Nothing;
        };
        if remaining.is_empty() {
            remaining.push(Tab::terminal(TabId(fresh.0.max(moved.id.0 + 1))));
        }
        content.finish_split(src, axis, remaining, moved);
        SplitResult::Done
    }

    /// An empty pane kept beside the focused one (drop target for tabs).
    pub(crate) fn park_pane(&mut self, ws: WorkspaceId) -> ParkOutcome {
        self.space_content(ws).map_or(ParkOutcome::Unchanged, |c| {
            c.park_empty(SplitAxis::Horizontal)
        })
    }

    pub(crate) fn close_tab(&mut self, ws: WorkspaceId, tab: TabId) -> Option<CloseOutcome> {
        let outcome = self.space_content(ws)?.close_tab(tab);
        self.activity.remove(&(ws, tab));
        self.attention.clear_tab(ws, tab);
        outcome
    }

    /// Ids of every tab in the focused pane except the active one.
    pub(crate) fn other_tabs(&self, ws: WorkspaceId) -> Vec<TabId> {
        let Some(leaf) = self.spaces.get(&ws).and_then(|s| s.content.focused_leaf()) else {
            return Vec::new();
        };
        let keep = leaf.active_tab().map(|t| t.id);
        leaf.tabs
            .iter()
            .map(|t| t.id)
            .filter(|id| Some(*id) != keep)
            .collect()
    }

    /// Show the tab `delta` places along in the focused pane, wrapping.
    pub(crate) fn step_tab(&mut self, ws: WorkspaceId, delta: isize) -> Option<TabId> {
        let leaf = self.space_content(ws)?.focused_leaf_mut()?;
        let n = leaf.tabs.len() as isize;
        if n == 0 {
            return None;
        }
        leaf.active = (leaf.active as isize + delta).rem_euclid(n) as usize;
        Some(leaf.tabs[leaf.active].id)
    }

    /// Make `tab` the active tab of its pane and that pane the focused one.
    pub(crate) fn select_tab(&mut self, ws: WorkspaceId, tab: TabId) -> bool {
        let Some(content) = self.space_content(ws) else {
            return false;
        };
        let Some((pane, index)) = content.find_tab(tab) else {
            return false;
        };
        let changed = content.focused != Some(pane)
            || content
                .root
                .as_ref()
                .and_then(|r| r.find_leaf(pane))
                .map(|l| l.active)
                != Some(index);
        if let Some(leaf) = content.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) {
            leaf.active = index;
        }
        content.focused = Some(pane);
        changed
    }

    /// Focus the pane `delta` leaves along (wrapping). Returns the new pane.
    pub(crate) fn step_pane(&mut self, ws: WorkspaceId, delta: isize) -> Option<PaneId> {
        let content = self.space_content(ws)?;
        let leaves = content.leaf_ids();
        let here = content
            .focused
            .and_then(|f| leaves.iter().position(|l| *l == f))?;
        let next = leaves[(here as isize + delta).rem_euclid(leaves.len() as isize) as usize];
        content.focused = Some(next);
        Some(next)
    }

    /// Divider drag: `pos` is the cumulative fraction of the split's extent.
    pub(crate) fn set_divider(&mut self, ws: WorkspaceId, key: PaneId, pos: f32) -> bool {
        self.space_content(ws)
            .and_then(|c| c.root.as_mut())
            .is_some_and(|root| root.set_boundary_for_split(key, pos))
    }

    /// The workspace's terminal tabs in pane order, each with its index in
    /// its pane (the number the chip label uses).
    pub(crate) fn terminal_ids(&self, ws: WorkspaceId) -> Vec<(TabId, usize)> {
        let mut out = Vec::new();
        if let Some(root) = self.spaces.get(&ws).and_then(|s| s.content.root.as_ref()) {
            root.for_each_leaf(&mut |leaf| {
                for (index, tab) in leaf.tabs.iter().enumerate() {
                    if matches!(tab.kind, TabKind::Terminal) {
                        out.push((tab.id, index));
                    }
                }
            });
        }
        out
    }

    /// Name the phone whose grid a terminal has (None = the Mac's again).
    /// True when the chip text changed.
    pub(crate) fn set_phone_device(
        &mut self,
        ws: WorkspaceId,
        tab: TabId,
        device: Option<String>,
    ) -> bool {
        match self.tab_mut(ws, tab) {
            Some(t) if t.phone_device != device => {
                t.phone_device = device;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn tab(&self, ws: WorkspaceId, tab: TabId) -> Option<&Tab> {
        let content = &self.spaces.get(&ws)?.content;
        let (pane, index) = content.find_tab(tab)?;
        content.root.as_ref()?.find_leaf(pane)?.tabs.get(index)
    }

    pub(crate) fn tab_mut(&mut self, ws: WorkspaceId, tab: TabId) -> Option<&mut Tab> {
        let content = &mut self.spaces.get_mut(&ws)?.content;
        let (pane, index) = content.find_tab(tab)?;
        content
            .root
            .as_mut()?
            .find_leaf_mut(pane)?
            .tabs
            .get_mut(index)
    }
}
