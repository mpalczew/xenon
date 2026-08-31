//! Live pane-tree mutations: open, close, split, move, focus.

use super::*;
use live::fix_active_idx;
use xenon_core::{PaneId, TabId};

enum DropTabOutcome {
    FocusPane(PaneId),
    Unsplit,
    Empty,
}

/// Which content surface should receive find commands (palette path).
pub(super) enum FindSurface {
    Terminal(Entity<TerminalView>),
    Editor(Entity<EditorView>),
    None,
}

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
            root.for_each_terminal(&mut |_, v| {
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

    /// Surface that owns ⌘F for the command palette (active tab preferred).
    pub(super) fn active_find_surface(&self) -> FindSurface {
        match self.active_content().and_then(|c| c.active_tab()) {
            Some(LiveTab::Terminal { view, .. }) => FindSurface::Terminal(view.clone()),
            Some(LiveTab::Editor { view, .. }) => FindSurface::Editor(view.clone()),
            None => {
                if let Some(editor) = self.active_editor() {
                    FindSurface::Editor(editor)
                } else if let Some(terminal) = self.active_terminal() {
                    FindSurface::Terminal(terminal)
                } else {
                    FindSurface::None
                }
            }
        }
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
        self.nav_sync_active(id, cx);
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
        if let Some(tab_id) = self
            .contents
            .get(&id)
            .and_then(|c| c.active_tab())
            .map(|t| t.id())
        {
            self.nav_visit(tab_id, cx);
        }
        cx.notify();
    }

    pub(crate) fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_gpui_leaf(window, cx);
        self.browser_focused = false;
        self.add_terminal(cx);
        if let Some(terminal) = self.active_terminal() {
            self.deferred.last_font_pane = FontPane::Terminal;
            terminal.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    pub(crate) fn open_editor(&mut self, path: PathBuf, focus: bool, cx: &mut Context<Self>) {
        if let Err(error) = self.open_editor_at(path, focus, None, cx) {
            log::error!("open failed: {error}");
        }
    }

    pub(crate) fn open_editor_at(
        &mut self,
        path: PathBuf,
        focus: bool,
        at: Option<(u32, u32)>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let Some(id) = self.active else {
            anyhow::bail!("no active workspace for {}", path.display());
        };
        if focus {
            self.nav_sync_active(id, cx);
        }
        if let Some(content) = self.contents.get(&id)
            && let Some(root) = &content.root
            && let Some((pane, idx)) = root.find_editor_path(&path)
        {
            let tab_id = root
                .find_leaf(pane)
                .and_then(|l| l.tabs.get(idx))
                .map(|t| t.id());
            if let Some(c) = self.contents.get_mut(&id) {
                if let Some(leaf) = c.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) {
                    leaf.active = idx;
                }
                c.focused = Some(pane);
            }
            if let Some((row, col)) = at
                && let Some(view) = self
                    .contents
                    .get(&id)
                    .and_then(|content| content.root.as_ref())
                    .and_then(|root| root.find_leaf(pane))
                    .and_then(|leaf| leaf.tabs.get(idx))
                    .and_then(LiveTab::as_editor)
            {
                view.update(cx, |editor, cx| {
                    editor.set_cursor_position(row, col, cx);
                });
            }
            self.touch_recent_file(id, &path);
            self.finder = None;
            if focus {
                self.deferred.pending_focus = Some(FocusPane::Editor);
                if let Some(tab_id) = tab_id {
                    self.nav_visit(tab_id, cx);
                }
            }
            if self.file_browser.is_open() {
                self.reveal_active_file(cx);
            }
            cx.notify();
            return Ok(());
        }

        match EditorView::build(path.clone(), focus, cx) {
            Ok(view) => {
                if let Some((row, col)) = at {
                    view.update(cx, |editor, cx| {
                        editor.set_cursor_position(row, col, cx);
                    });
                }
                self.wire_editor_selection(&view, cx);
                self.lsp_attach_editor(id, &view, cx);
                let name = file_name(&path);
                let content = self.contents.entry(id).or_default();
                let tab_id = content.next_tab_id();
                let tab = LiveTab::Editor {
                    id: tab_id,
                    path: path.clone(),
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
                self.touch_recent_file(id, &path);
                self.save_layout(id);
                self.finder = None;
                if focus {
                    self.deferred.pending_focus = Some(FocusPane::Editor);
                    self.nav_visit(tab_id, cx);
                }
                if self.file_browser.is_open() {
                    self.reveal_active_file(cx);
                }
                cx.notify();
                Ok(())
            }
            Err(error) => Err(error),
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
        let Some(tab) = (|| {
            let content = self.contents.get_mut(&id)?;
            let leaf = content.root.as_mut().and_then(|r| r.find_leaf_mut(pane))?;
            if index >= leaf.tabs.len() {
                return None;
            }
            leaf.active = index;
            content.focused = Some(pane);
            Some(leaf.tabs[index].clone())
        })() else {
            return;
        };
        let tab_id = tab.id();
        self.browser_focused = false;
        match tab {
            LiveTab::Terminal { view, .. } => {
                self.clear_tab_attention(id, tab_id, cx);
                self.deferred.last_font_pane = FontPane::Terminal;
                view.read(cx).focus_handle(cx).focus(window, cx);
            }
            LiveTab::Editor { path, view, .. } => {
                self.touch_recent_file(id, &path);
                self.deferred.last_font_pane = FontPane::Editor;
                view.update(cx, |view, cx| view.sync_from_disk(cx));
                view.read(cx).focus_handle(cx).focus(window, cx);
            }
        }
        self.nav_visit(tab_id, cx);
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

    /// Session leaf follows a surface the user is actually using (click/type).
    pub(super) fn adopt_focused_pane(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(content) = self.contents.get_mut(&id) else {
            return;
        };
        if content
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(pane))
            .is_none()
        {
            return;
        }
        if content.focused == Some(pane) && !self.browser_focused {
            return;
        }
        content.focused = Some(pane);
        self.browser_focused = false;
        cx.notify();
    }

    pub(super) fn adopt_tab_as_focused(
        &mut self,
        workspace: WorkspaceId,
        tab: TabId,
        cx: &mut Context<Self>,
    ) {
        let pane = {
            let Some(content) = self.contents.get_mut(&workspace) else {
                return;
            };
            let Some((pane, idx)) = content.root.as_ref().and_then(|r| r.find_tab(tab)) else {
                return;
            };
            if let Some(leaf) = content.root.as_mut().and_then(|r| r.find_leaf_mut(pane)) {
                leaf.active = idx;
            }
            if self.active != Some(workspace) {
                content.focused = Some(pane);
                return;
            }
            pane
        };
        self.adopt_focused_pane(pane, cx);
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
        let closed_editor_path = self
            .contents
            .get(&workspace)
            .and_then(|content| content.root.as_ref())
            .and_then(|root| {
                root.find_tab(tab).and_then(|(pane, index)| {
                    root.find_leaf(pane)
                        .and_then(|leaf| leaf.tabs.get(index))
                        .and_then(|tab| match tab {
                            LiveTab::Editor { path, .. } => Some(path.clone()),
                            LiveTab::Terminal { .. } => None,
                        })
                })
            });
        let outcome = {
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
                DropTabOutcome::FocusPane(pane)
            } else if content.leaf_ids().len() <= 1 {
                content.root = None;
                content.focused = None;
                DropTabOutcome::Empty
            } else {
                content.unsplit_empty(pane);
                DropTabOutcome::Unsplit
            }
        };
        if let Some(path) = closed_editor_path {
            self.lsp_detach_path(&path);
        }
        self.attention.clear_tab(workspace, tab);
        self.nav_prune_tab(workspace, tab);
        self.save_layout(workspace);
        match outcome {
            DropTabOutcome::FocusPane(pane) => {
                if let Some(window) = window {
                    self.focus_leaf_active(pane, window, cx);
                }
            }
            DropTabOutcome::Unsplit => {
                if let Some(window) = window
                    && let Some(fid) = self.contents.get(&workspace).and_then(|c| c.focused)
                {
                    self.focus_leaf_active(fid, window, cx);
                }
            }
            DropTabOutcome::Empty => self.focus_after_teardown(window, cx),
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
