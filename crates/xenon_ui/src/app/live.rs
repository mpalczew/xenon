//! Runtime pane tree: mixed terminal/editor tabs per leaf.

use super::*;
use xenon_core::{
    ContentLayout, PaneContent, PaneId, PaneLeaf, PaneTab, PaneTree, TabId, TabState,
};

/// One open surface in a leaf.
#[derive(Clone)]
pub(crate) enum LiveTab {
    Terminal {
        id: TabId,
        view: Entity<TerminalView>,
    },
    Editor {
        id: TabId,
        path: PathBuf,
        name: String,
        view: Entity<EditorView>,
    },
    /// Read-only git diff of one workspace-relative path. Not persisted.
    Diff {
        id: TabId,
        path: String,
        name: String,
        view: Entity<crate::diff_view::DiffView>,
    },
}

impl LiveTab {
    pub(crate) fn id(&self) -> TabId {
        match self {
            Self::Terminal { id, .. } | Self::Editor { id, .. } | Self::Diff { id, .. } => *id,
        }
    }

    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal { .. })
    }

    pub(crate) fn editor_path(&self) -> Option<&Path> {
        match self {
            Self::Editor { path, .. } => Some(path.as_path()),
            _ => None,
        }
    }

    pub(crate) fn as_terminal(&self) -> Option<&Entity<TerminalView>> {
        match self {
            Self::Terminal { view, .. } => Some(view),
            _ => None,
        }
    }

    pub(crate) fn as_editor(&self) -> Option<&Entity<EditorView>> {
        match self {
            Self::Editor { view, .. } => Some(view),
            _ => None,
        }
    }
}

impl PaneTab for LiveTab {
    fn tab_id(&self) -> TabId {
        self.id()
    }
}

pub(crate) type LiveLeaf = PaneLeaf<LiveTab>;
pub(crate) type LiveNode = PaneTree<LiveTab>;
pub(crate) type LiveContent = PaneContent<LiveTab>;

/// Lookups that need to see inside GPUI tabs.
pub(crate) trait LiveNodeExt {
    fn find_editor_path(&self, path: &Path) -> Option<(PaneId, usize)>;
    fn for_each_terminal(&self, f: &mut dyn FnMut(TabId, &Entity<TerminalView>));
    fn for_each_editor(&self, f: &mut dyn FnMut(&Entity<EditorView>, &Path));
    fn find_diff_path(&self, path: &str) -> Option<(PaneId, usize)>;
    fn diff_views(&self) -> Vec<Entity<crate::diff_view::DiffView>>;
    fn find_leaf_mut_with_editor(&mut self, view: &Entity<EditorView>) -> Option<&mut LiveLeaf>;
}

impl LiveNodeExt for LiveNode {
    fn find_editor_path(&self, path: &Path) -> Option<(PaneId, usize)> {
        self.find_position(&|t| t.editor_path() == Some(path))
    }

    fn for_each_terminal(&self, f: &mut dyn FnMut(TabId, &Entity<TerminalView>)) {
        self.for_each_leaf(&mut |leaf| {
            for tab in &leaf.tabs {
                if let LiveTab::Terminal { id, view } = tab {
                    f(*id, view);
                }
            }
        });
    }

    fn for_each_editor(&self, f: &mut dyn FnMut(&Entity<EditorView>, &Path)) {
        self.for_each_leaf(&mut |leaf| {
            for tab in &leaf.tabs {
                if let LiveTab::Editor { view, path, .. } = tab {
                    f(view, path);
                }
            }
        });
    }

    fn find_diff_path(&self, path: &str) -> Option<(PaneId, usize)> {
        self.find_position(&|t| t.diff_path() == Some(path))
    }

    fn diff_views(&self) -> Vec<Entity<crate::diff_view::DiffView>> {
        let mut views = Vec::new();
        self.for_each_leaf(&mut |leaf| {
            views.extend(leaf.tabs.iter().filter_map(LiveTab::as_diff).cloned());
        });
        views
    }

    fn find_leaf_mut_with_editor(&mut self, view: &Entity<EditorView>) -> Option<&mut LiveLeaf> {
        self.find_leaf_mut_where(&|t| t.as_editor().is_some_and(|v| v == view))
    }
}

pub(crate) trait LiveContentExt {
    fn has_editor(&self) -> bool;
    fn has_terminal(&self) -> bool;
    fn snapshot_layout(&self) -> ContentLayout;
}

impl LiveContentExt for LiveContent {
    fn has_editor(&self) -> bool {
        let mut any = false;
        if let Some(root) = &self.root {
            root.for_each_editor(&mut |_, _| any = true);
        }
        any
    }

    fn has_terminal(&self) -> bool {
        let mut any = false;
        if let Some(root) = &self.root {
            root.for_each_terminal(&mut |_, _| any = true);
        }
        any
    }

    fn snapshot_layout(&self) -> ContentLayout {
        self.snapshot(&snapshot_tab)
    }
}

fn snapshot_tab(tab: &LiveTab) -> Option<TabState> {
    Some(match tab {
        LiveTab::Terminal { id, .. } => TabState::Terminal {
            remote_session: None,
            id: *id,
            cwd: PathBuf::from("."),
        },
        LiveTab::Editor { id, path, .. } => TabState::Editor {
            id: *id,
            path: path.clone(),
            cursor: xenon_core::Point::default(),
            scroll_top: 0,
        },
        LiveTab::Diff { .. } => return None,
    })
}

/// Payload for dragging a tab between panes.
#[derive(Clone, Debug)]
pub(crate) struct DragTab {
    pub workspace: WorkspaceId,
    pub tab: TabId,
}
