//! Live pane-tree mutations: open, close, split, move, focus.

use super::*;
use live::fix_active_idx;
use xenon_core::{PaneId, TabId};

impl XenonApp {
    pub(crate) fn active_content(&self) -> Option<&LiveContent> {
        self.active.and_then(|id| self.contents.get(&id))
    }

    pub(crate) fn active_terminal(&self) -> Option<Entity<TerminalView>> {
        let content = self.active_content()?;
        if let Some(LiveTab::Terminal { view, .. }) = content.active_tab() {
            return Some(view.clone());
        }
        if let Some(leaf) = content.focused_leaf()
            && let Some(v) = leaf.tabs.iter().find_map(|t| t.as_terminal())
        {
            return Some(v.clone());
        }
        let mut found = None;
        if let Some(root) = content.root.as_ref() {
            root.for_each_terminal(&mut |v| {
                if found.is_none() {
                    found = Some(v.clone());
                }
            });
        }
        found
    }

    pub(crate) fn active_editor(&self) -> Option<Entity<EditorView>> {
        let content = self.active_content()?;
        if let Some(LiveTab::Editor { view, .. }) = content.active_tab() {
            return Some(view.clone());
        }
        if let Some(leaf) = content.focused_leaf()
            && let Some(v) = leaf.tabs.iter().rev().find_map(|t| t.as_editor())
        {
            return Some(v.clone());
        }
        let mut found = None;
        if let Some(root) = content.root.as_ref() {
            root.for_each_editor(&mut |v, _| {
                found = Some(v.clone());
            });
        }
        found
    }

    pub(crate) fn has_editor(&self) -> bool {
        self.active_content().is_some_and(|c| c.has_editor())
    }

    pub(crate) fn active_editor_is_previewing(&self, cx: &App) -> bool {
        self.active_editor()
            .is_some_and(|view| view.read(cx).is_previewing())
    }

    pub(crate) fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.active_editor() {
            view.update(cx, |view, cx| view.toggle_preview(cx));
        }
    }

    pub(crate) fn save_active_editor(&self, cx: &mut Context<Self>) {
        if let Some(editor) = self.active_editor() {
            editor.update(cx, |editor, cx| editor.save(cx));
        }
    }

    pub(crate) fn add_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(root) = self.workspace_root(id) else {
            return;
        };
        let view = self.spawn_terminal(root, id, cx);
        let content = self.contents.entry(id).or_default();
        let tab_id = content.next_tab_id();
        let tab = LiveTab::Terminal { id: tab_id, view };
        if content.root.is_none() {
            let pane = content.next_pane_id();
            content.root = Some(LiveNode::Leaf(LiveLeaf {
                id: pane,
                tabs: vec![tab],
                active: 0,
            }));
            content.focused = Some(pane);
        } else if let Some(leaf) = content.focused_leaf_mut() {
            leaf.tabs.push(tab);
            leaf.active = leaf.tabs.len() - 1;
        } else if let Some(first) = content.leaf_ids().first().copied() {
            content.focused = Some(first);
            if let Some(leaf) = content.focused_leaf_mut() {
                leaf.tabs.push(tab);
                leaf.active = leaf.tabs.len() - 1;
            }
        }
        self.save_layout(id);
        cx.notify();
    }

    pub(crate) fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browser_focused = false;
        self.add_terminal(cx);
        if let Some(terminal) = self.active_terminal() {
            self.deferred.last_font_pane = FontPane::Terminal;
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    pub(crate) fn open_editor(&mut self, path: PathBuf, focus: bool, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            log::warn!("open_editor: no active workspace for {}", path.display());
            return;
        };
        if let Some(content) = self.contents.get(&id)
            && let Some(root) = &content.root
            && let Some((pane, idx)) = root.find_editor_path(&path)
        {
            if let Some(c) = self.contents.get_mut(&id) {
                if let Some(leaf) = c.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) {
                    leaf.active = idx;
                }
                c.focused = Some(pane);
            }
            self.finder = None;
            if focus {
                self.deferred.pending_focus = Some(FocusPane::Editor);
            }
            if self.file_browser.is_open() {
                self.reveal_active_file(cx);
            }
            cx.notify();
            return;
        }

        match EditorView::build(path.clone(), focus, cx) {
            Ok(view) => {
                self.wire_editor_selection(&view, cx);
                let name = file_name(&path);
                let content = self.contents.entry(id).or_default();
                let tab_id = content.next_tab_id();
                let tab = LiveTab::Editor {
                    id: tab_id,
                    path,
                    name,
                    view,
                };
                if content.root.is_none() {
                    let pane = content.next_pane_id();
                    content.root = Some(LiveNode::Leaf(LiveLeaf {
                        id: pane,
                        tabs: vec![tab],
                        active: 0,
                    }));
                    content.focused = Some(pane);
                } else if let Some(leaf) = content.focused_leaf_mut() {
                    leaf.tabs.push(tab);
                    leaf.active = leaf.tabs.len() - 1;
                } else if let Some(first) = content.leaf_ids().first().copied() {
                    content.focused = Some(first);
                    if let Some(leaf) = content.focused_leaf_mut() {
                        leaf.tabs.push(tab);
                        leaf.active = leaf.tabs.len() - 1;
                    }
                }
                self.save_layout(id);
                self.finder = None;
                if focus {
                    self.deferred.pending_focus = Some(FocusPane::Editor);
                }
                if self.file_browser.is_open() {
                    self.reveal_active_file(cx);
                }
                cx.notify();
            }
            Err(error) => log::error!("open failed: {error}"),
        }
    }

    pub(crate) fn activate_tab_in_pane(
        &mut self,
        pane: PaneId,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.active else {
            return;
        };
        let Some(content) = self.contents.get_mut(&id) else {
            return;
        };
        let Some(leaf) = content.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) else {
            return;
        };
        if index >= leaf.tabs.len() {
            return;
        }
        leaf.active = index;
        content.focused = Some(pane);
        self.browser_focused = false;
        let tab = leaf.tabs[index].clone();
        match tab {
            LiveTab::Terminal { view, .. } => {
                self.deferred.last_font_pane = FontPane::Terminal;
                view.read(cx).focus_handle(cx).focus(window, cx);
            }
            LiveTab::Editor { view, .. } => {
                self.deferred.last_font_pane = FontPane::Editor;
                view.update(cx, |view, cx| view.sync_from_disk(cx));
                view.read(cx).focus_handle(cx).focus(window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn focus_leaf_active(
        &mut self,
        pane: PaneId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self
            .active
            .and_then(|id| self.contents.get(&id))
            .and_then(|c| c.root.as_ref()?.find_leaf(pane))
            .map(|l| l.active)
            .unwrap_or(0);
        self.activate_tab_in_pane(pane, index, window, cx);
    }

    /// Close by tab id (dirty guard for editors).
    pub(crate) fn close_tab_id(&mut self, tab: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        if let Some(content) = self.contents.get(&id)
            && let Some(root) = &content.root
            && let Some((pane, idx)) = root.find_tab(tab)
            && let Some(leaf) = root.find_leaf(pane)
            && let Some(LiveTab::Editor {
                path, name, view, ..
            }) = leaf.tabs.get(idx)
            && view.read(cx).is_dirty()
        {
            self.prompt_unsaved(
                vec![crate::app::dirty_close::DirtyTab {
                    name: name.clone(),
                    view: view.clone(),
                }],
                crate::app::dirty_close::DirtyClose::Tab {
                    workspace: id,
                    path: path.clone(),
                },
                window,
                cx,
            );
            return;
        }
        self.drop_tab(id, tab, Some(window), cx);
    }

    pub(super) fn drop_tab(
        &mut self,
        workspace: WorkspaceId,
        tab: TabId,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let Some(content) = self.contents.get_mut(&workspace) else {
            return;
        };
        let Some(root) = content.root.as_mut() else {
            return;
        };
        let Some((pane, idx)) = root.find_tab(tab) else {
            return;
        };
        let Some(leaf) = root.find_leaf_mut(pane) else {
            return;
        };
        leaf.tabs.remove(idx);
        if !leaf.tabs.is_empty() {
            fix_active_idx(&mut leaf.active, idx, leaf.tabs.len());
            content.focused = Some(pane);
            self.save_layout(workspace);
            if let Some(window) = window {
                self.focus_leaf_active(pane, window, cx);
            }
            cx.notify();
            return;
        }
        if content.leaf_ids().len() <= 1 {
            content.root = None;
            content.focused = None;
            self.save_layout(workspace);
            cx.notify();
            return;
        }
        content.unsplit_empty(pane);
        self.save_layout(workspace);
        if let Some(window) = window
            && let Some(fid) = self.contents.get(&workspace).and_then(|c| c.focused)
        {
            self.focus_leaf_active(fid, window, cx);
        }
        cx.notify();
    }

    pub(super) fn drop_editor_tab_by_path(
        &mut self,
        id: WorkspaceId,
        path: &Path,
        cx: &mut Context<Self>,
    ) {
        let tab = self.contents.get(&id).and_then(|c| {
            let (pane, idx) = c.root.as_ref()?.find_editor_path(path)?;
            c.root
                .as_ref()?
                .find_leaf(pane)?
                .tabs
                .get(idx)
                .map(|t| t.id())
        });
        if let Some(tab) = tab {
            self.drop_tab(id, tab, None, cx);
        }
    }
}
