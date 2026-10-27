//! Split / move / edge-drop for the live pane tree.

use super::*;
mod lookup;
use live::fix_active_idx;
use xenon_core::{DropEdge, PaneId, SplitAxis, TabId};

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
            if !content
                .root
                .as_ref()
                .is_some_and(|r| r.can_split(src_pane, axis))
            {
                xenon_design_system::show_toast(super::toasts::split_limit(), cx);
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
            let tab_id = self
                .contents
                .get(&ws)
                .map(|c| c.next_tab_id())
                .unwrap_or(TabId(1));
            let tab_id = TabId(tab_id.0.max(moved.id().0.saturating_add(1)));
            let view = self.spawn_terminal_for(root_path, (ws, tab_id), cx);
            vec![LiveTab::Terminal { id: tab_id, view }]
        } else {
            remaining
        };

        let Some(content) = self.contents.get_mut(&ws) else {
            return;
        };
        let new_pane = content.next_pane_id();
        let replacement = LiveNode::pair(
            axis,
            LiveNode::Leaf(LiveLeaf {
                id: src_pane,
                tabs: source_tabs,
                active: 0,
                parked: false,
            }),
            LiveNode::Leaf(LiveLeaf {
                id: new_pane,
                tabs: vec![moved],
                active: 0,
                parked: false,
            }),
        );
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
        self.move_tab_to_pane_at(tab, dest, usize::MAX, window, cx);
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn move_tab_to_pane_at(
        &mut self,
        tab: TabId,
        dest: PaneId,
        mut insert_at: usize,
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
        if content
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(dest))
            .is_none()
        {
            return;
        }
        if src_pane == dest && insert_at != usize::MAX && src_idx < insert_at {
            insert_at -= 1;
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
            let at = if insert_at == usize::MAX {
                leaf.tabs.len()
            } else {
                insert_at.min(leaf.tabs.len())
            };
            leaf.tabs.insert(at, tab_state);
            leaf.active = at;
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
        let can_split = self
            .contents
            .get(&ws)
            .and_then(|c| c.root.as_ref())
            .is_some_and(|r| r.can_split(target, edge.axis()));
        if !can_split {
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
            let Some(root_path) = root_path else {
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
            let term_id = self
                .contents
                .get(&ws)
                .map(|c| c.next_tab_id())
                .unwrap_or(TabId(1));
            let term_id = TabId(term_id.0.max(tab_state.id().0.saturating_add(1)));
            let view = self.spawn_terminal_for(root_path, (ws, term_id), cx);
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
        let split = LiveNode::pair(edge.axis(), first, second);
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
                    let Some(root) = this
                        .locate_editor(&view)
                        .and_then(|(workspace, _)| this.workspace_root(workspace))
                    else {
                        return;
                    };
                    let Some(ide) = this.services.ide.as_ref() else {
                        return;
                    };
                    ide.notify_selection(
                        &root,
                        &SelectionSnapshot {
                            path: path.clone(),
                            text: text.clone(),
                            start_line: *start_line,
                            start_character: *start_character,
                            end_line: *end_line,
                            end_character: *end_character,
                        },
                    );
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
                EditorEvent::OpenLink { path, target } => {
                    this.open_doc_link(path, target, cx);
                }
                EditorEvent::FullScreen { on } => {
                    this.writing_full_screen = *on;
                    cx.notify();
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
