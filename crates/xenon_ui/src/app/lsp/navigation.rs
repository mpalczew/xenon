use std::path::PathBuf;

use gpui::{Context, PromptLevel};
use lsp_types::Position;
use xenon_lsp::{Location, char_col_to_utf16};

use super::super::nav_history::NavEntry;
use super::XenonApp;

#[derive(Clone)]
struct DefinitionOrigin {
    workspace: xenon_core::WorkspaceId,
    entry: NavEntry,
}

impl XenonApp {
    pub(crate) fn go_to_definition(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.active_editor() else {
            return;
        };
        let Some((row, col)) = view.read(cx).cursor_position() else {
            return;
        };
        let path = view.read(cx).path().to_path_buf();
        self.request_definition(path, row, col, cx);
    }

    pub(crate) fn request_definition(
        &mut self,
        path: PathBuf,
        row: u32,
        col: u32,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.active else {
            return;
        };
        let Some((root, family)) = self.lsp_context(&path) else {
            return;
        };
        let Some(view) = self.editor_for_path(&path) else {
            return;
        };
        let Some(line) = view.read(cx).line_text(row) else {
            return;
        };
        let origin = DefinitionOrigin {
            workspace,
            entry: NavEntry::Editor {
                path: path.clone(),
                row,
                col,
            },
        };
        let position = Position::new(row, char_col_to_utf16(&line, col as usize));
        let Ok(request) = self.lsp.definition(&root, family, &path, position) else {
            return;
        };
        cx.spawn(async move |app, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    request
                        .recv()
                        .and_then(xenon_lsp::LspHost::decode_definition)
                })
                .await;
            let Ok(locations) = result else {
                return;
            };
            app.update_in(cx, |app, window, cx| {
                app.choose_definition(origin, locations, window, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn next_diagnostic(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(view) = self.active_editor() else {
            return;
        };
        let path = view.read(cx).path().to_path_buf();
        let current = view.read(cx).cursor_position().unwrap_or_default();
        let Some((_, diagnostics)) = self.lsp_diagnostics.get(&path) else {
            return;
        };
        let mut positions = diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.range.start.0,
                    self.utf16_col_for_path(
                        &path,
                        diagnostic.range.start.0,
                        diagnostic.range.start.1,
                        cx,
                    ),
                )
            })
            .collect::<Vec<_>>();
        positions.sort_unstable();
        let target = if forward {
            positions
                .iter()
                .copied()
                .find(|position| *position > current)
                .or_else(|| positions.first().copied())
        } else {
            positions
                .iter()
                .copied()
                .rev()
                .find(|position| *position < current)
                .or_else(|| positions.last().copied())
        };
        if let Some((row, col)) = target {
            view.update(cx, |editor, cx| editor.set_cursor_position(row, col, cx));
        }
    }

    fn choose_definition(
        &mut self,
        origin: DefinitionOrigin,
        locations: Vec<Location>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        match locations.len() {
            0 => {}
            1 => self.apply_definition(origin, locations[0].clone(), cx),
            _ => {
                let labels = locations
                    .iter()
                    .map(|location| {
                        format!(
                            "{}:{}",
                            location.path.display(),
                            location.line.saturating_add(1)
                        )
                    })
                    .collect::<Vec<_>>();
                let choices = labels.iter().map(String::as_str).collect::<Vec<_>>();
                let answer =
                    window.prompt(PromptLevel::Info, "Choose definition", None, &choices, cx);
                cx.spawn(async move |app, cx| {
                    if let Ok(index) = answer.await
                        && let Some(location) = locations.get(index).cloned()
                    {
                        app.update(cx, |app, cx| {
                            app.apply_definition(origin, location, cx);
                        })
                        .ok();
                    }
                })
                .detach();
            }
        }
    }

    fn apply_definition(
        &mut self,
        origin: DefinitionOrigin,
        location: Location,
        cx: &mut Context<Self>,
    ) {
        let col =
            self.utf16_col_for_path(&location.path, location.line, location.character_utf16, cx);
        if self
            .open_editor_at(location.path.clone(), true, Some((location.line, col)), cx)
            .is_ok()
        {
            self.nav_record_definition(
                origin.workspace,
                origin.entry,
                NavEntry::Editor {
                    path: location.path,
                    row: location.line,
                    col,
                },
            );
        }
    }
}
