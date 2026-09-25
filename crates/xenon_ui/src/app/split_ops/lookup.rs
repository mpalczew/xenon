use super::*;

impl LiveNode {
    pub(super) fn find_leaf_mut_with_editor(
        &mut self,
        view: &Entity<EditorView>,
    ) -> Option<&mut LiveLeaf> {
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
    pub(in crate::app) fn locate_terminal(
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
    pub(in crate::app) fn locate_editor(
        &self,
        view: &Entity<EditorView>,
    ) -> Option<(WorkspaceId, TabId)> {
        let ids: Vec<WorkspaceId> = self.contents.keys().copied().collect();
        for id in ids {
            if let Some(tab) = self.tab_id_for_editor(id, view) {
                return Some((id, tab));
            }
        }
        None
    }
}
