//! Sidebar Git section: collapse, size, keyboard cursor, and opening a
//! file's read-only diff in the focused pane.

use gpui::ScrollHandle;

use super::*;
use crate::diff_view::DiffView;
use crate::git_dirt::GitFile;

const ANIMATION: Duration = Duration::from_millis(180);

pub(crate) struct GitSectionState {
    pub(crate) collapsed: bool,
    pub(crate) closing: bool,
    animation: Option<Task<()>>,
    /// Pinned body height; `None` sizes to content.
    pub(crate) height: Option<f32>,
    /// The section owns arrow keys (like `browser_focused` for Files).
    pub(crate) focused: bool,
    pub(crate) cursor: usize,
    pub(crate) scroll: ScrollHandle,
}

impl GitSectionState {
    pub(super) fn new(settings: &xenon_store::AppSettings) -> Self {
        Self {
            collapsed: settings.git_collapsed,
            closing: false,
            animation: None,
            height: settings.git_section_height,
            focused: false,
            cursor: 0,
            scroll: ScrollHandle::new(),
        }
    }
}

impl XenonApp {
    /// Status of the active workspace; `None` for SSH and non-git folders.
    pub(crate) fn active_git(&self) -> Option<&crate::git_dirt::GitStatus> {
        self.workspace_git(self.active?)
    }

    pub(crate) fn clear_sidebar_focus(&mut self) {
        self.browser_focused = false;
        self.git.focused = false;
    }

    pub(crate) fn git_cursor(&self) -> usize {
        let len = self.active_git().map_or(0, |git| git.files.len());
        self.git.cursor.min(len.saturating_sub(1))
    }

    pub(crate) fn toggle_git_section(&mut self, cx: &mut Context<Self>) {
        if self.git.closing {
            self.git.animation.take();
            self.git.closing = false;
            cx.notify();
            return;
        }
        if self.git.collapsed {
            self.git.collapsed = false;
            self.persist_section_prefs();
            cx.notify();
            return;
        }
        self.git.closing = true;
        self.git.animation = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ANIMATION).await;
            this.update(cx, |this, cx| {
                if this.git.closing {
                    this.git.collapsed = true;
                    this.git.closing = false;
                    this.persist_section_prefs();
                    cx.notify();
                }
                this.git.animation.take();
            })
            .ok();
        }));
        cx.notify();
    }

    /// ⌘⌥G: focus the Git section (expanding it); again to fold it away and
    /// return to the pane.
    pub(super) fn toggle_git_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.git.focused {
            self.git.focused = false;
            if !self.git.collapsed {
                self.toggle_git_section(cx);
            }
            self.focus_workspace_leaf(Some(window), cx);
            return;
        }
        self.focus_git(window, cx);
    }

    pub(super) fn focus_git(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_git().is_none() {
            return;
        }
        self.sidebar_collapsed = false;
        if self.git.collapsed || self.git.closing {
            self.git.animation.take();
            self.git.closing = false;
            self.git.collapsed = false;
            self.persist_section_prefs();
        }
        self.browser_focused = false;
        self.git.focused = true;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Git-section keyboard while `git.focused`.
    pub(super) fn on_git_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.git.focused {
            return false;
        }
        let len = self.active_git().map_or(0, |git| git.files.len());
        match event.keystroke.key.as_str() {
            "escape" => {
                self.git.focused = false;
                self.focus_workspace_leaf(Some(window), cx);
                true
            }
            "up" | "down" if len > 0 => {
                let step: isize = if event.keystroke.key == "up" { -1 } else { 1 };
                let next = (self.git_cursor() as isize + step).clamp(0, len as isize - 1);
                self.git.cursor = next as usize;
                crate::palette::reveal_selected(&self.git.scroll, self.git.cursor);
                cx.notify();
                true
            }
            "enter" if len > 0 => {
                let path = self
                    .active_git()
                    .and_then(|git| git.files.get(self.git_cursor()))
                    .map(|file| file.path.clone());
                if let Some(path) = path {
                    self.open_git_diff(&path, cx);
                }
                true
            }
            _ => false,
        }
    }

    /// Open (or re-activate) the diff tab for `path` in the focused pane.
    pub(crate) fn open_git_diff(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(file) = self
            .workspace_git(id)
            .and_then(|git| git.files.iter().find(|f| f.path == path))
            .cloned()
        else {
            return;
        };
        let Some(root) = self.workspace_root(id) else {
            return;
        };
        self.nav_sync_active(id, cx);
        self.clear_sidebar_focus();
        let existing = self
            .contents
            .get(&id)
            .and_then(|c| c.root.as_ref()?.find_diff_path(path));
        if let Some(slot) = existing {
            self.reuse_diff_tab(id, slot, file, cx);
        } else {
            self.push_diff_tab(id, root, file, cx);
        }
        self.deferred.pending_focus = Some(FocusOwner::Diff);
        self.save_layout(id);
        cx.notify();
    }

    fn reuse_diff_tab(
        &mut self,
        id: WorkspaceId,
        (pane, index): (xenon_core::PaneId, usize),
        file: GitFile,
        cx: &mut Context<Self>,
    ) {
        let Some(content) = self.contents.get_mut(&id) else {
            return;
        };
        let view = content.root.as_mut().and_then(|root| {
            let leaf = root.find_leaf_mut(pane)?;
            leaf.active = index;
            leaf.tabs.get(index)?.as_diff().cloned()
        });
        content.focused = Some(pane);
        if let Some(view) = view {
            view.update(cx, |view, cx| view.reload(Some(file), cx));
        }
    }

    fn push_diff_tab(
        &mut self,
        id: WorkspaceId,
        root: PathBuf,
        file: GitFile,
        cx: &mut Context<Self>,
    ) {
        let name = file.name().to_string();
        let path = file.path.clone();
        let view = cx.new(|cx| DiffView::new(root, file, cx));
        let content = self.contents.entry(id).or_default();
        let tab = LiveTab::Diff {
            id: content.next_tab_id(),
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
                parked: false,
            }));
            content.focused = Some(pane);
            return;
        }
        if content.focused_leaf().is_none() {
            content.focused = content.leaf_ids().first().copied();
        }
        if let Some(leaf) = content.focused_leaf_mut() {
            leaf.parked = false;
            leaf.tabs.push(tab);
            leaf.active = leaf.tabs.len() - 1;
        }
    }

    /// ⌘⌥⇧D: cycle Auto → Split → Unified on the active diff tab.
    pub(super) fn cycle_diff_layout(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self
            .active_content()
            .and_then(|c| c.active_tab())
            .and_then(LiveTab::as_diff)
            .cloned()
        else {
            return;
        };
        view.update(cx, |view, cx| view.cycle_layout(cx));
    }

    /// Visual scenes: open a diff tab with a prepared document, in the
    /// focused pane or (`beside`) a new pane to its right.
    #[cfg(feature = "visual-tests")]
    pub(crate) fn open_scene_diff(
        &mut self,
        file: GitFile,
        mode: crate::diff_model::LayoutMode,
        doc: crate::diff_model::DiffDoc,
        beside: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.active else {
            return;
        };
        let name = file.name().to_string();
        let path = file.path.clone();
        let view = cx.new(|cx| DiffView::with_doc(file, mode, doc, cx));
        let tab_id = self
            .contents
            .get(&id)
            .map(LiveContent::next_tab_id)
            .unwrap_or(TabId(1));
        let tab = LiveTab::Diff {
            id: tab_id,
            path,
            name,
            view,
        };
        if beside {
            self.split_right_with_tab(tab, true);
        } else if let Some(leaf) = self
            .contents
            .get_mut(&id)
            .and_then(LiveContent::focused_leaf_mut)
        {
            leaf.tabs.push(tab);
            leaf.active = leaf.tabs.len() - 1;
        }
        self.deferred.pending_focus = Some(FocusOwner::Diff);
        cx.notify();
    }

    /// Reload open diff tabs of these workspaces (after an FS burst).
    pub(super) fn refresh_diff_tabs(
        &mut self,
        ids: &std::collections::HashSet<WorkspaceId>,
        cx: &mut Context<Self>,
    ) {
        for id in ids {
            let views = self
                .contents
                .get(id)
                .and_then(|c| c.root.as_ref())
                .map(LiveNode::diff_views)
                .unwrap_or_default();
            for view in views {
                let file = self.workspace_git(*id).and_then(|git| {
                    let path = view.read(cx).path();
                    git.files.iter().find(|f| f.path == path).cloned()
                });
                view.update(cx, |view, cx| view.reload(file, cx));
            }
        }
    }
}

impl LiveTab {
    pub(crate) fn diff_path(&self) -> Option<&str> {
        match self {
            Self::Diff { path, .. } => Some(path.as_str()),
            _ => None,
        }
    }

    pub(crate) fn as_diff(&self) -> Option<&Entity<crate::diff_view::DiffView>> {
        match self {
            Self::Diff { view, .. } => Some(view),
            _ => None,
        }
    }
}

impl LiveNode {
    pub(crate) fn find_diff_path(&self, path: &str) -> Option<(xenon_core::PaneId, usize)> {
        let mut found = None;
        self.for_each_leaf(&mut |leaf| {
            if found.is_none() {
                found = leaf
                    .tabs
                    .iter()
                    .position(|t| t.diff_path() == Some(path))
                    .map(|i| (leaf.id, i));
            }
        });
        found
    }

    pub(crate) fn diff_views(&self) -> Vec<Entity<crate::diff_view::DiffView>> {
        let mut views = Vec::new();
        self.for_each_leaf(&mut |leaf| {
            views.extend(leaf.tabs.iter().filter_map(LiveTab::as_diff).cloned());
        });
        views
    }
}
