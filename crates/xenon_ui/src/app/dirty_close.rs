//! Refuse to discard unsaved editor buffers without an explicit choice.

use super::*;

/// What to run after the user saves or discards unsaved buffers.
#[derive(Clone)]
pub(super) enum DirtyClose {
    /// Drop one tab (path-keyed; index may have moved).
    Tab {
        workspace: WorkspaceId,
        path: PathBuf,
    },
    Workspace(WorkspaceId),
}

struct DirtyTab {
    name: String,
    view: Entity<EditorView>,
}

impl XenonApp {
    /// Close tab: prompt when dirty, else drop immediately.
    pub(crate) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.active else {
            return;
        };
        let Some(stack) = self.editors.get(&id) else {
            return;
        };
        if index >= stack.tabs.len() {
            return;
        }
        let tab = &stack.tabs[index];
        if !tab.view.read(cx).is_dirty() {
            self.drop_editor_tab(id, index, cx);
            return;
        }
        let path = tab.path.clone();
        self.prompt_unsaved(
            vec![DirtyTab {
                name: tab.name.clone(),
                view: tab.view.clone(),
            }],
            DirtyClose::Tab {
                workspace: id,
                path,
            },
            window,
            cx,
        );
    }

    /// Close workspace after confirming dirty editors (Save All / Don't Save / Cancel).
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
        let Some(stack) = self.editors.get(&id) else {
            return out;
        };
        for tab in &stack.tabs {
            if tab.view.read(cx).is_dirty() {
                out.push(DirtyTab {
                    name: tab.name.clone(),
                    view: tab.view.clone(),
                });
            }
        }
        out
    }

    fn prompt_unsaved(
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
