//! Open editor tabs into the focused leaf or a new split sibling.

use super::*;
use xenon_core::{DEFAULT_SPLIT_RATIO, MAX_NEST_DEPTH, SplitAxis, TabId};

impl XenonApp {
    pub(super) fn build_workspace_editor(
        path: PathBuf,
        root: &Path,
        autofocus: bool,
        cx: &mut gpui::App,
    ) -> anyhow::Result<Entity<EditorView>> {
        if path == root.join(".xenon/worklist.md") {
            EditorView::build_worklist(path, autofocus, cx)
        } else {
            Ok(EditorView::build(path, autofocus, cx)?)
        }
    }

    pub(crate) fn open_worklist(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.active else {
            self.show_worklist_notice("Open a workspace first", cx);
            return;
        };
        self.worklist_capture_visible = None;
        let Some(root) = self.workspace_root(workspace) else {
            return;
        };
        let path = root.join(".xenon/worklist.md");
        if let Err(error) = self.open_editor_at(path, true, None, cx) {
            log::error!("worklist open failed: {error}");
        }
    }

    pub(crate) fn open_editor(&mut self, path: PathBuf, focus: bool, cx: &mut Context<Self>) {
        if let Err(error) = self.open_editor_at(path, focus, None, cx) {
            log::error!("open failed: {error}");
        }
    }

    /// Open `path` in a new pane to the right of the focused leaf.
    /// Already-open files and max nest depth fall back to [`Self::open_editor`].
    pub(crate) fn open_editor_beside(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            self.open_editor(path, true, cx);
            return;
        };
        let already = self
            .contents
            .get(&id)
            .and_then(|c| c.root.as_ref())
            .and_then(|r| r.find_editor_path(&path))
            .is_some();
        let can_split = self.contents.get(&id).is_some_and(|c| {
            let Some(pane) = c.focused else {
                return false;
            };
            let depth = c
                .root
                .as_ref()
                .and_then(|r| r.nest_depth(pane))
                .unwrap_or(0);
            depth < MAX_NEST_DEPTH
                && c.root
                    .as_ref()
                    .and_then(|r| r.find_leaf(pane))
                    .is_some_and(|leaf| !leaf.tabs.is_empty())
        });
        if already || !can_split {
            self.open_editor(path, true, cx);
            return;
        }
        let built = self
            .workspace_root(id)
            .ok_or_else(|| anyhow::anyhow!("workspace root unavailable"))
            .and_then(|root| Self::build_workspace_editor(path.clone(), &root, true, cx));
        match built {
            Ok(view) => {
                self.wire_editor_selection(&view, cx);
                self.lsp_attach_editor(id, &view, cx);
                let name = file_name(&path);
                let tab_id = self
                    .contents
                    .get(&id)
                    .map(|c| c.next_tab_id())
                    .unwrap_or(TabId(1));
                let tab = LiveTab::Editor {
                    id: tab_id,
                    path: path.clone(),
                    name,
                    view,
                };
                if !self.split_right_with_tab(tab, true) {
                    log::error!("open beside failed for {}", path.display());
                    return;
                }
                self.touch_recent_file(id, &path);
                self.finder = None;
                self.deferred.pending_focus = Some(FocusOwner::Editor);
                self.nav_visit(tab_id, cx);
                if self.file_browser.is_open() {
                    self.reveal_active_file(cx);
                }
                cx.notify();
            }
            Err(error) => log::error!("open failed: {error}"),
        }
    }

    #[allow(clippy::too_many_lines)]
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
        let is_worklist = self
            .workspace_root(id)
            .is_some_and(|root| path == root.join(".xenon/worklist.md"));
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
                if focus {
                    c.focused = Some(pane);
                }
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
                self.deferred.pending_focus = Some(FocusOwner::Editor);
                if let Some(tab_id) = tab_id {
                    self.nav_visit(tab_id, cx);
                }
            }
            if self.file_browser.is_open() && !is_worklist {
                self.reveal_active_file(cx);
            }
            cx.notify();
            return Ok(());
        }

        let built = self
            .workspace_root(id)
            .ok_or_else(|| anyhow::anyhow!("workspace root unavailable"))
            .and_then(|root| Self::build_workspace_editor(path.clone(), &root, focus, cx));
        match built {
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
                        parked: false,
                    }));
                    content.focused = Some(pane);
                } else if let Some(leaf) = content.focused_leaf_mut() {
                    leaf.parked = false;
                    leaf.tabs.push(tab);
                    leaf.active = leaf.tabs.len() - 1;
                } else if let Some(first) = content.leaf_ids().first().copied() {
                    content.focused = Some(first);
                    if let Some(leaf) = content.focused_leaf_mut() {
                        leaf.parked = false;
                        leaf.tabs.push(tab);
                        leaf.active = leaf.tabs.len() - 1;
                    }
                }
                self.touch_recent_file(id, &path);
                self.save_layout(id);
                self.finder = None;
                if focus {
                    self.deferred.pending_focus = Some(FocusOwner::Editor);
                    self.nav_visit(tab_id, cx);
                }
                if self.file_browser.is_open() && !is_worklist {
                    self.reveal_active_file(cx);
                }
                cx.notify();
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Split the focused leaf to the right; `tab` is the new sibling's only tab.
    /// False if there is nothing to split or the nest cap is hit.
    pub(crate) fn split_right_with_tab(&mut self, tab: LiveTab, focus_new: bool) -> bool {
        let Some(ws) = self.active else {
            return false;
        };
        let Some(content) = self.contents.get_mut(&ws) else {
            return false;
        };
        let Some(src_pane) = content.focused else {
            return false;
        };
        let depth = content
            .root
            .as_ref()
            .and_then(|r| r.nest_depth(src_pane))
            .unwrap_or(0);
        if depth >= MAX_NEST_DEPTH {
            return false;
        }
        let Some(src) = content
            .root
            .as_ref()
            .and_then(|r| r.find_leaf(src_pane))
            .cloned()
        else {
            return false;
        };
        if src.tabs.is_empty() {
            return false;
        }
        let new_pane = content.next_pane_id();
        let replacement = LiveNode::Split {
            axis: SplitAxis::Horizontal,
            ratio: DEFAULT_SPLIT_RATIO,
            first: Box::new(LiveNode::Leaf(src)),
            second: Box::new(LiveNode::Leaf(LiveLeaf {
                id: new_pane,
                tabs: vec![tab],
                active: 0,
                parked: false,
            })),
        };
        if let Some(root) = content.root.as_mut()
            && !root.replace_leaf(src_pane, replacement)
        {
            return false;
        }
        if focus_new {
            content.focused = Some(new_pane);
        }
        self.save_layout(ws);
        true
    }
}
