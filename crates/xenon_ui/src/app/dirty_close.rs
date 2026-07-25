//! Refuse to discard unsaved editor buffers without an explicit choice.

use super::*;

/// What to run after the user saves or discards unsaved buffers.
#[derive(Clone)]
pub(crate) enum DirtyClose {
    Tab {
        workspace: WorkspaceId,
        path: PathBuf,
    },
    /// Close a list of tabs (e.g. Close Other Tabs).
    Tabs {
        workspace: WorkspaceId,
        tabs: Vec<TabId>,
    },
    Workspace(WorkspaceId),
}

pub(crate) struct DirtyTab {
    pub name: String,
    pub view: Entity<EditorView>,
}

impl XenonApp {
    pub(crate) fn close_workspace(
        &mut self,
        id: WorkspaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dirty = self.dirty_tabs_in_workspace(id, cx);
        if dirty.is_empty() {
            self.force_close_workspace(id, Some(window), cx);
            return;
        }
        self.prompt_unsaved(dirty, DirtyClose::Workspace(id), window, cx);
    }

    fn dirty_tabs_in_workspace(&self, id: WorkspaceId, cx: &App) -> Vec<DirtyTab> {
        let mut out = Vec::new();
        let Some(content) = self.contents.get(&id) else {
            return out;
        };
        if let Some(root) = &content.root {
            root.for_each_editor(&mut |view, path| {
                if view.read(cx).is_dirty() {
                    out.push(DirtyTab {
                        name: file_name(path),
                        view: view.clone(),
                    });
                }
            });
        }
        out
    }

    pub(crate) fn prompt_unsaved(
        &mut self,
        dirty: Vec<DirtyTab>,
        after: DirtyClose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (message, detail, save_label) = unsaved_copy(&dirty);
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            detail.as_deref(),
            &[save_label, "Don't Save", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            let Ok(choice) = answer.await else {
                return;
            };
            this.update(cx, |this, cx| match choice {
                0 => {
                    for tab in &dirty {
                        tab.view.update(cx, |editor, cx| editor.save(cx));
                    }
                    this.finish_dirty_close(after, cx);
                }
                1 => this.finish_dirty_close(after, cx),
                _ => {}
            })
            .ok();
        })
        .detach();
    }

    fn finish_dirty_close(&mut self, after: DirtyClose, cx: &mut Context<Self>) {
        match after {
            DirtyClose::Tab { workspace, path } => {
                self.drop_editor_tab_by_path(workspace, &path, cx);
            }
            DirtyClose::Tabs { workspace, tabs } => {
                self.drop_tabs(workspace, &tabs, None, cx);
            }
            DirtyClose::Workspace(id) => self.force_close_workspace(id, None, cx),
        }
    }
}

fn unsaved_copy(dirty: &[DirtyTab]) -> (String, Option<String>, &'static str) {
    match dirty {
        [] => unreachable!("prompt only when dirty"),
        [one] => (
            format!(
                "Do you want to save the changes you made to \"{}\"?",
                one.name
            ),
            Some("Your changes will be lost if you don't save them.".into()),
            "Save",
        ),
        many => {
            let mut names: Vec<&str> = many.iter().map(|t| t.name.as_str()).collect();
            names.sort_unstable();
            names.dedup();
            let listed = names.join(", ");
            (
                format!(
                    "You have {} unsaved files. Save them before closing?",
                    many.len()
                ),
                Some(format!(
                    "{listed}\n\nYour changes will be lost if you don't save them."
                )),
                "Save All",
            )
        }
    }
}
