//! Agent/CLI file open: location + pane, never steal terminal focus.

use super::*;
use xenon_core::{MAX_NEST_DEPTH, TabId};
use xenon_store::{OpenFileSpec, OpenPane};

impl XenonApp {
    pub fn open_cli_file(&mut self, spec: OpenFileSpec, cx: &mut Context<Self>) {
        let path = spec
            .path
            .canonicalize()
            .unwrap_or_else(|_| spec.path.clone());
        if path.is_dir() {
            self.register_workspace(path, cx);
            return;
        }
        if !path.is_file() {
            log::warn!("open_cli_file: not a file: {}", path.display());
            return;
        }
        if let Some(workspace_id) = self.workspace_for_path(&path) {
            self.focus_workspace(workspace_id, cx);
        } else if self.active.is_none()
            && let Some(parent) = path.parent()
        {
            self.register_workspace(parent.to_path_buf(), cx);
        }
        let spec = OpenFileSpec { path, ..spec };
        match spec.pane {
            OpenPane::Focused => self.open_spec_in_focused(&spec, cx),
            OpenPane::SplitRight => self.open_spec_split_right(&spec, cx),
            OpenPane::Sibling => self.open_spec_sibling(&spec, cx),
        }
    }

    fn open_spec_in_focused(&mut self, spec: &OpenFileSpec, cx: &mut Context<Self>) {
        let at = spec_cursor(spec);
        let focus = takes_editor_focus(spec.pane);
        if let Err(error) = self.open_editor_at(spec.path.clone(), focus, at, cx) {
            log::error!("open failed: {error}");
            return;
        }
        self.apply_range(spec, cx);
    }

    fn open_spec_split_right(&mut self, spec: &OpenFileSpec, cx: &mut Context<Self>) {
        if self.editor_already_open(&spec.path) {
            self.open_spec_in_focused(spec, cx);
            return;
        }
        if self.try_split_with_file(spec, cx) {
            return;
        }
        self.open_spec_in_focused(spec, cx);
    }

    fn open_spec_sibling(&mut self, spec: &OpenFileSpec, cx: &mut Context<Self>) {
        if self.editor_already_open(&spec.path) {
            self.open_spec_in_focused(spec, cx);
            return;
        }
        let sibling = self.active.and_then(|id| {
            let content = self.contents.get(&id)?;
            let focused = content.focused?;
            content.root.as_ref()?.sibling_of(focused)
        });
        if let Some(sibling) = sibling {
            let orig = self
                .active
                .and_then(|id| self.contents.get(&id).and_then(|c| c.focused));
            if let Some(content) = self.active.and_then(|id| self.contents.get_mut(&id)) {
                content.focused = Some(sibling);
            }
            self.open_spec_in_focused(spec, cx);
            if let Some(content) = self.active.and_then(|id| self.contents.get_mut(&id)) {
                content.focused = orig;
            }
            return;
        }
        self.open_spec_split_right(spec, cx);
    }

    fn editor_already_open(&self, path: &Path) -> bool {
        self.active.is_some_and(|id| {
            self.contents
                .get(&id)
                .and_then(|c| c.root.as_ref())
                .and_then(|r| r.find_editor_path(path))
                .is_some()
        })
    }

    fn try_split_with_file(&mut self, spec: &OpenFileSpec, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.active else {
            return false;
        };
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
        if !can_split {
            return false;
        }
        let Some(root) = self.workspace_root(id) else {
            return false;
        };
        let Ok(view) = Self::build_workspace_editor(spec.path.clone(), &root, false, cx) else {
            return false;
        };
        apply_location_to_view(&view, spec, cx);
        self.wire_editor_selection(&view, cx);
        self.lsp_attach_editor(id, &view, cx);
        let tab_id = self
            .contents
            .get(&id)
            .map(|c| c.next_tab_id())
            .unwrap_or(TabId(1));
        let tab = LiveTab::Editor {
            id: tab_id,
            path: spec.path.clone(),
            name: file_name(&spec.path),
            view,
        };
        if !self.split_right_with_tab(tab, false) {
            return false;
        }
        self.touch_recent_file(id, &spec.path);
        cx.notify();
        true
    }

    fn apply_range(&mut self, spec: &OpenFileSpec, cx: &mut Context<Self>) {
        let Some(end_line) = spec.end_line else {
            return;
        };
        let Some(start) = spec_cursor(spec) else {
            return;
        };
        let Some(id) = self.active else {
            return;
        };
        let Some(view) = self
            .contents
            .get(&id)
            .and_then(|c| c.root.as_ref())
            .and_then(|r| r.find_editor_path(&spec.path))
            .and_then(|(pane, idx)| {
                self.contents
                    .get(&id)?
                    .root
                    .as_ref()?
                    .find_leaf(pane)?
                    .tabs
                    .get(idx)
                    .and_then(LiveTab::as_editor)
            })
        else {
            return;
        };
        apply_range_to_view(view, start, end_line, cx);
    }
}

/// `--pane focused` is the one policy that should take GPUI focus.
pub(super) fn takes_editor_focus(pane: OpenPane) -> bool {
    matches!(pane, OpenPane::Focused)
}

fn spec_cursor(spec: &OpenFileSpec) -> Option<(u32, u32)> {
    spec.line.map(|line| {
        (
            line.saturating_sub(1),
            spec.column.unwrap_or(1).saturating_sub(1),
        )
    })
}

fn apply_location_to_view(
    view: &Entity<EditorView>,
    spec: &OpenFileSpec,
    cx: &mut Context<XenonApp>,
) {
    if let Some(end_line) = spec.end_line
        && let Some(start) = spec_cursor(spec)
    {
        apply_range_to_view(view, start, end_line, cx);
        return;
    }
    if let Some((row, col)) = spec_cursor(spec) {
        view.update(cx, |editor, cx| editor.set_cursor_position(row, col, cx));
    }
}

fn apply_range_to_view(
    view: &Entity<EditorView>,
    start: (u32, u32),
    end_line: u32,
    cx: &mut Context<XenonApp>,
) {
    let end_row = end_line.saturating_sub(1);
    let end_col = view
        .read(cx)
        .line_text(end_row)
        .map(|text| text.trim_end_matches(['\n', '\r']).chars().count() as u32)
        .unwrap_or(0);
    view.update(cx, |editor, cx| {
        editor.set_selection_range(start, (end_row, end_col), cx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_focused_pane_steals_editor_focus() {
        assert!(takes_editor_focus(OpenPane::Focused));
        assert!(!takes_editor_focus(OpenPane::Sibling));
        assert!(!takes_editor_focus(OpenPane::SplitRight));
    }
}
