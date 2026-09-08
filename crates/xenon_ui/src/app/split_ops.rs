//! Split / move / edge-drop for the live pane tree.

use super::*;
use live::fix_active_idx;
use xenon_core::{DropEdge, MAX_NEST_DEPTH, PaneId, SplitAxis, TabId};

impl XenonApp {
    /// Split focused leaf: move active tab to new sibling; spawn term if source empty.
    pub(crate) fn do_split(
        &mut self,
        axis: SplitAxis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.follow_gpui_leaf(window, cx);
        let Some(ws) = self.active else {
            return;
        };
        let root_path = self.workspace_root(ws);

        let (src_pane, moved, remaining, depth_ok) = {
            let Some(content) = self.contents.get_mut(&ws) else {
                return;
            };
            let Some(src_pane) = content.focused else {
                return;
            };
            let depth = content
                .root
                .as_ref()
                .and_then(|r| r.nest_depth(src_pane))
                .unwrap_or(0);
            if depth >= MAX_NEST_DEPTH {
                return;
            }
            let Some(leaf) = content
                .root
                .as_mut()
                .and_then(|r| r.find_leaf_mut(src_pane))
            else {
                return;
            };
            if leaf.tabs.is_empty() {
                return;
            }
            let idx = leaf.active.min(leaf.tabs.len() - 1);
            let moved = leaf.tabs.remove(idx);
            let remaining = std::mem::take(&mut leaf.tabs);
            (src_pane, moved, remaining, true)
        };
        let _ = depth_ok;

        let source_tabs = if remaining.is_empty() {
            let Some(root_path) = root_path else {
                // Restore moved tab.
                if let Some(content) = self.contents.get_mut(&ws)
                    && let Some(leaf) = content
                        .root
                        .as_mut()
                        .and_then(|r| r.find_leaf_mut(src_pane))
                {
                    leaf.tabs.push(moved);
                    leaf.active = 0;
                }
                return;
            };
            let view = self.spawn_terminal(root_path, ws, cx);
            let tab_id = self
                .contents
                .get(&ws)
                .map(|c| c.next_tab_id())
                .unwrap_or(TabId(1));
            vec![LiveTab::Terminal { id: tab_id, view }]
        } else {
            remaining
        };

        let Some(content) = self.contents.get_mut(&ws) else {
            return;
        };
        let new_pane = content.next_pane_id();
        let replacement = LiveNode::Split {
            axis,
            ratio: 0.5,
            first: Box::new(LiveNode::Leaf(LiveLeaf {
                id: src_pane,
                tabs: source_tabs,
                active: 0,
                parked: false,
            })),
            second: Box::new(LiveNode::Leaf(LiveLeaf {
                id: new_pane,
                tabs: vec![moved],
                active: 0,
                parked: false,
            })),
        };
        if let Some(root) = content.root.as_mut()
            && !root.replace_leaf(src_pane, replacement)
        {
            return;
        }
        content.focused = Some(new_pane);
        self.save_layout(ws);
        self.focus_leaf_active(new_pane, window, cx);
        cx.notify();
    }

    pub(crate) fn move_tab_to_pane(
        &mut self,
        tab: TabId,
        dest: PaneId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ws) = self.active else {
            return;
        };
        let Some(content) = self.contents.get_mut(&ws) else {
            return;
        };
        let Some((src_pane, src_idx)) = content.root.as_ref().and_then(|r| r.find_tab(tab)) else {
            return;
        };
        if src_pane == dest {
            if let Some(leaf) = content
                .root
                .as_mut()
                .and_then(|r| r.find_leaf_mut(src_pane))
            {
                leaf.active = src_idx;
            }
            content.focused = Some(src_pane);
            self.focus_leaf_active(src_pane, window, cx);
            return;
        }
        if content
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(dest))
            .is_none()
        {
            return;
        }
        let tab_state = {
            let leaf = content
                .root
                .as_mut()
                .unwrap()
                .find_leaf_mut(src_pane)
                .unwrap();
            let t = leaf.tabs.remove(src_idx);
            if !leaf.tabs.is_empty() {
                fix_active_idx(&mut leaf.active, src_idx, leaf.tabs.len());
            }
            t
        };
        {
            let leaf = content.root.as_mut().unwrap().find_leaf_mut(dest).unwrap();
            leaf.parked = false;
            leaf.tabs.push(tab_state);
            leaf.active = leaf.tabs.len() - 1;
        }
        if content
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(src_pane))
            .is_some_and(|l| l.tabs.is_empty())
        {
            content.unsplit_empty(src_pane);
        }
        if let Some(content) = self.contents.get_mut(&ws) {
            content.focused = Some(dest);
        }
        self.save_layout(ws);
        self.focus_leaf_active(dest, window, cx);
        cx.notify();
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub(crate) fn drop_tab_on_edge(
        &mut self,
        tab: TabId,
        target: PaneId,
        edge: DropEdge,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ws) = self.active else {
            return;
        };
        let root_path = self.workspace_root(ws);
        let depth = self
            .contents
            .get(&ws)
            .and_then(|c| c.root.as_ref())
            .and_then(|r| r.nest_depth(target))
            .unwrap_or(0);
        if depth >= MAX_NEST_DEPTH {
            self.move_tab_to_pane(tab, target, window, cx);
            return;
        }

        let (src_pane, src_idx) = {
            let Some(content) = self.contents.get(&ws) else {
                return;
            };
            let Some(found) = content.root.as_ref().and_then(|r| r.find_tab(tab)) else {
                return;
            };
            found
        };

        // Peel tab out of source. If that empties the leaf:
        // - same pane (edge-split the only tab): leave a fresh terminal there
        // - other pane: unsplit the empty source
        let tab_state = {
            let content = self.contents.get_mut(&ws).unwrap();
            let leaf = content
                .root
                .as_mut()
                .unwrap()
                .find_leaf_mut(src_pane)
                .unwrap();
            let t = leaf.tabs.remove(src_idx);
            if !leaf.tabs.is_empty() {
                fix_active_idx(&mut leaf.active, src_idx, leaf.tabs.len());
            }
            t
        };

        let source_empty = self
            .contents
            .get(&ws)
            .and_then(|c| c.root.as_ref())
            .and_then(|r| r.find_leaf(src_pane))
            .is_some_and(|l| l.tabs.is_empty());

        if source_empty && src_pane == target {
            // Only-tab edge drop on its own pane: same as toolbar split.
            let Some(root_path) = root_path else {
                // Restore tab.
                if let Some(content) = self.contents.get_mut(&ws)
                    && let Some(leaf) = content
                        .root
                        .as_mut()
                        .and_then(|r| r.find_leaf_mut(src_pane))
                {
                    leaf.tabs.push(tab_state);
                    leaf.active = 0;
                }
                return;
            };
            let view = self.spawn_terminal(root_path, ws, cx);
            let term_id = self
                .contents
                .get(&ws)
                .map(|c| c.next_tab_id())
                .unwrap_or(TabId(1));
            if let Some(content) = self.contents.get_mut(&ws)
                && let Some(leaf) = content
                    .root
                    .as_mut()
                    .and_then(|r| r.find_leaf_mut(src_pane))
            {
                leaf.tabs.push(LiveTab::Terminal { id: term_id, view });
                leaf.active = 0;
            }
        } else if source_empty && let Some(content) = self.contents.get_mut(&ws) {
            content.unsplit_empty(src_pane);
        }

        let Some(content) = self.contents.get_mut(&ws) else {
            return;
        };
        // Target may have been removed if it was the empty source we unsplit
        // (only when src != target). Re-resolve.
        let target_leaf = match content.root.as_ref().and_then(|r| r.find_leaf(target)) {
            Some(l) => l.clone(),
            None => return,
        };
        let new_pane = content.next_pane_id();
        let new_leaf = LiveLeaf {
            id: new_pane,
            tabs: vec![tab_state],
            active: 0,
            parked: false,
        };
        let (first, second) = if edge.tab_in_first() {
            (LiveNode::Leaf(new_leaf), LiveNode::Leaf(target_leaf))
        } else {
            (LiveNode::Leaf(target_leaf), LiveNode::Leaf(new_leaf))
        };
        let split = LiveNode::Split {
            axis: edge.axis(),
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        };
        if let Some(root) = content.root.as_mut() {
            root.replace_leaf(target, split);
        }
        content.focused = Some(new_pane);
        self.save_layout(ws);
        self.focus_leaf_active(new_pane, window, cx);
        cx.notify();
    }

    pub(super) fn wire_editor_selection(
        &mut self,
        view: &Entity<EditorView>,
        cx: &mut Context<Self>,
    ) {
        self._selection_subs
            .push(cx.subscribe(view, |this, view, event, cx| match event {
                EditorEvent::SelectionChanged {
                    path,
                    text,
                    start_line,
                    start_character,
                    end_line,
                    end_character,
                } => {
                    let Some(ide) = this.services.ide.as_ref() else {
                        return;
                    };
                    ide.notify_selection(&SelectionSnapshot {
                        path: path.clone(),
                        text: text.clone(),
                        start_line: *start_line,
                        start_character: *start_character,
                        end_line: *end_line,
                        end_character: *end_character,
                    });
                }
                EditorEvent::RequestClose { force } => {
                    this.handle_editor_request_close(&view, *force, cx);
                }
                EditorEvent::PathChanged { path } => {
                    this.rebind_editor_tab_path(&view, path.clone(), cx);
                }
                EditorEvent::BufferChanged { path, text } => {
                    this.lsp_buffer_changed(path, text.clone(), cx);
                }
                EditorEvent::Saved { path } => {
                    this.lsp_saved(path);
                }
                EditorEvent::CursorMoved { path, row, col } => {
                    this.lsp_cursor_moved(path.clone(), *row, *col, cx);
                }
                EditorEvent::GoToDefinition { path, row, col } => {
                    this.request_definition(path.clone(), *row, *col, cx);
                }
                EditorEvent::Focused => {
                    if let Some((workspace, tab)) = this.locate_editor(&view) {
                        this.adopt_tab_as_focused(workspace, tab, cx);
                    }
                }
            }));
    }

    fn handle_editor_request_close(
        &mut self,
        view: &Entity<EditorView>,
        _force: bool,
        cx: &mut Context<Self>,
    ) {
        // Dirty checks happen in the editor (`:q` vs `:q!`); shell just drops the tab.
        let Some(id) = self.active else {
            return;
        };
        let Some(tab_id) = self.tab_id_for_editor(id, view) else {
            return;
        };
        self.drop_tab(id, tab_id, None, cx);
    }

    fn rebind_editor_tab_path(
        &mut self,
        view: &Entity<EditorView>,
        new_path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.active else {
            return;
        };
        let Some(content) = self.contents.get_mut(&id) else {
            return;
        };
        let Some(root) = content.root.as_mut() else {
            return;
        };
        if let Some(leaf) = root.find_leaf_mut_with_editor(view)
            && let Some(LiveTab::Editor { path, name, .. }) = leaf
                .tabs
                .iter_mut()
                .find(|t| t.as_editor().is_some_and(|v| v == view))
        {
            *path = new_path.clone();
            *name = file_name(&new_path);
        }
        self.touch_recent_file(id, &new_path);
        self.save_layout(id);
        cx.notify();
    }

    fn tab_id_for_editor(
        &self,
        workspace: WorkspaceId,
        view: &Entity<EditorView>,
    ) -> Option<xenon_core::TabId> {
        let root = self.contents.get(&workspace)?.root.as_ref()?;
        for pane in root.leaf_ids() {
            if let Some(leaf) = root.find_leaf(pane) {
                for t in &leaf.tabs {
                    if t.as_editor().is_some_and(|v| v == view) {
                        return Some(t.id());
                    }
                }
            }
        }
        None
    }
}

impl LiveNode {
    fn find_leaf_mut_with_editor(&mut self, view: &Entity<EditorView>) -> Option<&mut LiveLeaf> {
        match self {
            Self::Leaf(l) => {
                if l.tabs
                    .iter()
                    .any(|t| t.as_editor().is_some_and(|v| v == view))
                {
                    Some(l)
                } else {
                    None
                }
            }
            Self::Split { first, second, .. } => first
                .find_leaf_mut_with_editor(view)
                .or_else(|| second.find_leaf_mut_with_editor(view)),
        }
    }
}

impl XenonApp {
    pub(crate) fn sync_editors_for_paths(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        for content in self.contents.values() {
            if let Some(root) = &content.root {
                root.for_each_editor(&mut |view, open| {
                    if paths.iter().any(|p| p == open || open.starts_with(p)) {
                        view.update(cx, |view, cx| view.sync_from_disk(cx));
                    }
                });
            }
        }
    }

    /// Find (workspace, tab) for a terminal entity.
    pub(super) fn locate_terminal(
        &self,
        view: &Entity<TerminalView>,
    ) -> Option<(WorkspaceId, TabId)> {
        for (id, content) in &self.contents {
            let Some(root) = &content.root else {
                continue;
            };
            let mut found = None;
            root.for_each_terminal(&mut |tab, term| {
                if found.is_none() && term == view {
                    found = Some(tab);
                }
            });
            if let Some(tab) = found {
                return Some((*id, tab));
            }
        }
        None
    }

    /// Find (workspace, tab) for an editor entity.
    pub(super) fn locate_editor(&self, view: &Entity<EditorView>) -> Option<(WorkspaceId, TabId)> {
        let ids: Vec<WorkspaceId> = self.contents.keys().copied().collect();
        for id in ids {
            if let Some(tab) = self.tab_id_for_editor(id, view) {
                return Some((id, tab));
            }
        }
        None
    }
}
