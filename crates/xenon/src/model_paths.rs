use xenon_core::{TabId, TabState, WorkspaceId};

use crate::model::{Model, TabKind};

impl Model {
    pub(crate) fn follow_rename(
        &mut self,
        old: &std::path::Path,
        new: &std::path::Path,
    ) -> Vec<(WorkspaceId, TabId)> {
        let mut changed = Vec::new();
        for (ws, space) in &mut self.spaces {
            let Some(root) = space.content.root.as_mut() else {
                continue;
            };
            root.for_each_leaf_mut(&mut |leaf| {
                for tab in &mut leaf.tabs {
                    if let TabKind::Kept(TabState::Editor { path, .. }) = &mut tab.kind
                        && !tab.modified
                        && let Some(moved) = xenon_finder::fileops::rebase(path, old, new)
                    {
                        *path = moved;
                        changed.push((*ws, tab.id));
                    }
                }
            });
        }
        changed
    }
}
