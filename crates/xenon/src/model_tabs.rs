use xenon_core::{
    CloseOutcome, DropEdge, PaneId, PaneLeaf, PaneTree, ParkOutcome, SplitAxis, TabId, TabState,
    WorkspaceId,
};

use crate::model::{Model, Tab, TabKind};

pub(crate) enum SplitResult {
    Done,
    NoRoom,
    Nothing,
}

impl Model {
    pub(super) fn space_content(
        &mut self,
        ws: WorkspaceId,
    ) -> Option<&mut xenon_core::PaneContent<Tab>> {
        self.spaces.get_mut(&ws).map(|s| &mut s.content)
    }

    pub(crate) fn new_terminal(&mut self, ws: WorkspaceId) -> Option<TabId> {
        self.push_tab(ws, Tab::terminal)
    }

    pub(super) fn push_tab(
        &mut self,
        ws: WorkspaceId,
        make: impl FnOnce(TabId) -> Tab,
    ) -> Option<TabId> {
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

    pub(crate) fn remove_empty_pane(&mut self, ws: WorkspaceId, pane: PaneId) -> bool {
        self.space_content(ws)
            .is_some_and(|content| content.remove_empty_leaf(pane))
    }

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
        let single = content
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(src))
            .is_some_and(|leaf| leaf.tabs.len() <= 1);
        if single {
            return match content.park_empty(axis) {
                ParkOutcome::Added(blank) => {
                    content.focused = Some(blank);
                    SplitResult::Done
                }
                ParkOutcome::NoRoom => SplitResult::NoRoom,
                _ => SplitResult::Nothing,
            };
        }
        let Some((moved, remaining)) = content.take_active_for_split(src) else {
            return SplitResult::Nothing;
        };
        content.finish_split(src, axis, remaining, moved);
        SplitResult::Done
    }

    pub(crate) fn park_pane(&mut self, ws: WorkspaceId) -> ParkOutcome {
        self.space_content(ws).map_or(ParkOutcome::Unchanged, |c| {
            c.park_empty(SplitAxis::Horizontal)
        })
    }

    /// A tab dragged to `index` of pane `dest` (its final position there).
    /// True when the tree changed.
    pub(crate) fn place_tab(
        &mut self,
        ws: WorkspaceId,
        tab: TabId,
        dest: PaneId,
        index: Option<usize>,
    ) -> bool {
        let Some(content) = self.space_content(ws) else {
            return false;
        };
        let Some((src, from)) = content.find_tab(tab) else {
            return false;
        };
        let at = match index {
            None => usize::MAX,
            Some(i) if src == dest && i > from => i + 1,
            Some(i) => i,
        };
        content.move_tab(tab, dest, at)
    }

    pub(crate) fn drop_on_edge(
        &mut self,
        ws: WorkspaceId,
        tab: TabId,
        target: PaneId,
        edge: DropEdge,
    ) -> bool {
        let Some(content) = self.space_content(ws) else {
            return false;
        };
        let room = content
            .root
            .as_ref()
            .is_some_and(|r| r.can_split(target, edge.axis()));
        if !room {
            return content.move_tab(tab, target, usize::MAX);
        }
        let Some((src, index)) = content.find_tab(tab) else {
            return false;
        };
        let Some(moved) = content.take_tab_at(src, index) else {
            return false;
        };
        let emptied = content
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(src))
            .is_some_and(|l| l.tabs.is_empty());
        if emptied && src == target {
            let next = content.next_tab_id();
            let fresh = TabId(next.0.max(moved.id.0 + 1));
            content.put_back(src, Tab::terminal(fresh));
        } else if emptied {
            content.unsplit_empty(src);
        }
        content.finish_edge_drop(target, edge, moved).is_some()
    }

    pub(crate) fn close_tab(&mut self, ws: WorkspaceId, tab: TabId) -> Option<CloseOutcome> {
        let outcome = self.space_content(ws)?.close_tab(tab);
        self.activity.remove(&(ws, tab));
        self.attention.clear_tab(ws, tab);
        outcome
    }

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

    pub(crate) fn step_tab(&mut self, ws: WorkspaceId, delta: isize) -> Option<TabId> {
        let leaf = self.space_content(ws)?.focused_leaf_mut()?;
        let n = leaf.tabs.len() as isize;
        if n == 0 {
            return None;
        }
        leaf.active = (leaf.active as isize + delta).rem_euclid(n) as usize;
        Some(leaf.tabs[leaf.active].id)
    }

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

    pub(crate) fn set_divider(&mut self, ws: WorkspaceId, key: PaneId, pos: f32) -> bool {
        self.space_content(ws)
            .and_then(|c| c.root.as_mut())
            .is_some_and(|root| root.set_boundary_for_split(key, pos))
    }

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
    /// True when the tab title changed.
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
