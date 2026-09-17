//! Tab close-others and path clipboard helpers for context menus.

use super::*;
use xenon_core::TabId;

impl XenonApp {
    /// Close every other tab in the same leaf as `keep` (dirty prompt if needed).
    pub(crate) fn close_other_tabs(
        &mut self,
        keep: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.active else {
            return;
        };
        let Some(others) = self.other_tabs_in_pane(workspace, keep) else {
            return;
        };
        if others.is_empty() {
            return;
        }
        let dirty = self.dirty_among_tabs(workspace, &others, cx);
        if dirty.is_empty() {
            self.drop_tabs(workspace, &others, Some(window), cx);
            return;
        }
        self.prompt_unsaved(
            dirty,
            crate::app::dirty_close::DirtyClose::Tabs {
                workspace,
                tabs: others,
            },
            window,
            cx,
        );
    }

    pub(crate) fn drop_tabs(
        &mut self,
        workspace: WorkspaceId,
        tabs: &[TabId],
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        for &tab in tabs {
            self.drop_tab(workspace, tab, None, cx);
        }
        if let Some(window) = window
            && let Some(pane) = self.contents.get(&workspace).and_then(|c| c.focused)
        {
            self.focus_leaf_active(pane, window, cx);
        }
        cx.notify();
    }

    fn other_tabs_in_pane(&self, workspace: WorkspaceId, keep: TabId) -> Option<Vec<TabId>> {
        let root = self.contents.get(&workspace)?.root.as_ref()?;
        let (pane, _) = root.find_tab(keep)?;
        let leaf = root.find_leaf(pane)?;
        Some(
            leaf.tabs
                .iter()
                .map(|t| t.id())
                .filter(|id| *id != keep)
                .collect(),
        )
    }

    fn dirty_among_tabs(
        &self,
        workspace: WorkspaceId,
        tabs: &[TabId],
        cx: &App,
    ) -> Vec<crate::app::dirty_close::DirtyTab> {
        let mut out = Vec::new();
        let Some(root) = self.contents.get(&workspace).and_then(|c| c.root.as_ref()) else {
            return out;
        };
        for &tab in tabs {
            let Some((pane, idx)) = root.find_tab(tab) else {
                continue;
            };
            let Some(leaf) = root.find_leaf(pane) else {
                continue;
            };
            if let Some(LiveTab::Editor { name, view, .. }) = leaf.tabs.get(idx)
                && view.read(cx).is_dirty()
            {
                out.push(crate::app::dirty_close::DirtyTab {
                    name: name.clone(),
                    view: view.clone(),
                });
            }
        }
        out
    }

    pub(crate) fn tab_editor_path(&self, tab: TabId) -> Option<PathBuf> {
        let id = self.active?;
        let root = self.contents.get(&id)?.root.as_ref()?;
        let (pane, idx) = root.find_tab(tab)?;
        root.find_leaf(pane)?
            .tabs
            .get(idx)?
            .editor_path()
            .map(|p| p.to_path_buf())
    }

    pub(crate) fn pane_tab_count(&self, tab: TabId) -> usize {
        let Some(id) = self.active else {
            return 0;
        };
        self.contents
            .get(&id)
            .and_then(|c| c.root.as_ref())
            .and_then(|root| {
                let (pane, _) = root.find_tab(tab)?;
                Some(root.find_leaf(pane)?.tabs.len())
            })
            .unwrap_or(0)
    }

    pub(crate) fn copy_path_relative(&self, path: &Path, cx: &mut Context<Self>) {
        let text = self
            .active
            .and_then(|id| self.workspace_root(id))
            .and_then(|root| path.strip_prefix(&root).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    }

    pub(crate) fn copy_path_abs(path: &Path, cx: &mut App) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
            path.to_string_lossy().into_owned(),
        ));
    }

    fn focused_tab_id(&self) -> Option<TabId> {
        Some(self.active_content()?.focused_leaf()?.active_tab()?.id())
    }

    fn focused_editor_path(&self) -> Option<PathBuf> {
        self.active_content()?
            .focused_leaf()?
            .active_tab()?
            .editor_path()
            .map(|path| path.to_path_buf())
            .or_else(|| {
                self.file_browser
                    .cursor()
                    .and_then(|_| self.tree_cursor_path_opt())
            })
    }

    fn tree_cursor_path_opt(&self) -> Option<PathBuf> {
        let rows = self.tree_rows();
        let i = self.file_browser.cursor()?;
        rows.get(i).map(|row| row.path.clone())
    }

    pub(crate) fn close_other_tabs_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.focused_tab_id() {
            self.close_other_tabs(tab, window, cx);
        }
    }

    pub(crate) fn copy_focused_path(&self, relative: bool, cx: &mut Context<Self>) {
        let Some(path) = self.focused_editor_path() else {
            return;
        };
        if relative {
            self.copy_path_relative(&path, cx);
        } else {
            Self::copy_path_abs(&path, cx);
        }
    }

    pub(crate) fn reveal_focused_path(&self) {
        if let Some(path) = self.focused_editor_path() {
            Self::reveal_in_finder(&path);
        }
    }

    pub(crate) fn open_focused_in_default_app(&self, cx: &mut App) {
        if let Some(path) = self.focused_editor_path() {
            cx.open_with_system(&path);
        }
    }

    /// macOS: `open -R` selects the file in Finder.
    pub(crate) fn reveal_in_finder(path: &Path) {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
}
