use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use gpui::{App, Context, Entity};
use lsp_types::Position;
use xenon_editor::{EditorDiagnostic, EditorDiagnosticSeverity, EditorView};
use xenon_lsp::{
    DiagnosticSeverity, DocumentHighlight, DocumentSnapshot, HostEvent, LanguageFamily,
    ServerConfig, char_col_to_utf16, utf16_to_char_col,
};

use super::XenonApp;

mod discovery;
mod navigation;
mod state;
use discovery::{collect_editors, executable_on_path, has_root_marker, language_for_path};
use state::LspDocumentState;
pub(super) use state::LspState;

impl XenonApp {
    pub(super) fn start_lsp_events(
        &mut self,
        receiver: mpsc::Receiver<HostEvent>,
        cx: &mut Context<Self>,
    ) {
        let receiver = Arc::new(Mutex::new(receiver));
        self.lsp.event_task = Some(cx.spawn(async move |app, cx| {
            loop {
                let receiver = Arc::clone(&receiver);
                let event = cx
                    .background_executor()
                    .spawn(async move { receiver.lock().ok()?.recv().ok() })
                    .await;
                let Some(event) = event else {
                    break;
                };
                if app
                    .update(cx, |app, cx| app.handle_lsp_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub(super) fn lsp_attach_editor(
        &mut self,
        workspace: xenon_core::WorkspaceId,
        view: &Entity<EditorView>,
        cx: &mut Context<Self>,
    ) {
        if !self.lsp.settings.enabled {
            return;
        }
        let path = view.read(cx).path().to_path_buf();
        let Some((family, _)) = language_for_path(&path) else {
            return;
        };
        let Some(root) = self.workspace_root(workspace) else {
            return;
        };
        if !has_root_marker(&path, &root, family) {
            return;
        }
        let Some(config) = self.server_config(family) else {
            view.update(cx, |editor, cx| {
                editor.set_lsp_status(Some(missing_server_message(family)), cx);
            });
            return;
        };
        if let Err(error) = self.lsp.host.start_server(root.clone(), family, config) {
            log::info!("LSP unavailable for {}: {error}", path.display());
            view.update(cx, |editor, cx| {
                editor.set_lsp_status(Some(missing_server_message(family)), cx);
            });
            return;
        }
        self.try_open_lsp_document(&root, family, view, cx);
    }

    pub(super) fn lsp_buffer_changed(&mut self, path: &Path, text: String, cx: &mut Context<Self>) {
        if let Some(document) = self.lsp.documents.get_mut(path) {
            document.diagnostics.clear();
            document.diagnostics_version = None;
        }
        self.apply_lsp_decorations(path, Vec::new(), cx);
        let Some((root, family)) = self.lsp_context(path) else {
            return;
        };
        let path = path.to_path_buf();
        let generation = self.bump_sync_generation(&path);
        let host = self.lsp.host.clone();
        cx.spawn(async move |app, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let sync_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { host.change_document(&root, family, &sync_path, text) })
                .await;
            app.update(cx, |app, _| {
                if let Some(document) = app.lsp.documents.get_mut(&path)
                    && document.sync_generation == generation
                    && let Ok(version) = result
                {
                    document.synced_version = Some(version);
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn lsp_saved(&self, path: &Path) {
        let Some((root, family)) = self.lsp_context(path) else {
            return;
        };
        let _ = self.lsp.host.save_document(&root, family, path);
    }

    pub(super) fn lsp_detach_path(&mut self, path: &Path) {
        if !self
            .lsp
            .documents
            .get(path)
            .is_some_and(|document| document.open)
        {
            return;
        }
        if let Some((root, family)) = self.lsp_context(path) {
            let _ = self.lsp.host.close_document(&root, family, path);
        }
        self.lsp.documents.remove(path);
    }

    pub(super) fn lsp_cursor_moved(
        &mut self,
        path: PathBuf,
        row: u32,
        col: u32,
        cx: &mut Context<Self>,
    ) {
        let generation = self.bump_lsp_generation(&path);
        let Some((root, family)) = self.lsp_context(&path) else {
            return;
        };
        let Some(view) = self.editor_for_path(&path) else {
            return;
        };
        let Some(line) = view.read(cx).line_text(row) else {
            return;
        };
        let position = Position::new(row, char_col_to_utf16(&line, col as usize));
        let host = self.lsp.host.clone();
        cx.spawn(async move |app, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(220))
                .await;
            let request = match host.document_highlight(&root, family, &path, position) {
                Ok(request) => request,
                Err(_) => return,
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    request
                        .recv()
                        .and_then(xenon_lsp::LspHost::decode_highlights)
                })
                .await;
            let Ok(highlights) = result else {
                return;
            };
            app.update(cx, |app, cx| {
                if app
                    .lsp
                    .documents
                    .get(&path)
                    .is_some_and(|document| document.request_generation == generation)
                {
                    app.apply_highlights(&path, highlights, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn handle_lsp_event(&mut self, event: HostEvent, cx: &mut Context<Self>) {
        match event {
            HostEvent::Diagnostics {
                path,
                version,
                diagnostics,
            } => {
                let document = self.lsp.documents.entry(path.clone()).or_default();
                let current_version = document.synced_version;
                if version
                    .zip(current_version)
                    .is_some_and(|(got, current)| got < current)
                {
                    return;
                }
                document.diagnostics_version = version.or(current_version);
                document.diagnostics = diagnostics;
                self.apply_lsp_decorations(&path, Vec::new(), cx);
            }
            HostEvent::ServerState {
                root,
                family,
                state: xenon_lsp::ClientState::Running,
            } => {
                self.set_server_status(&root, family, None, cx);
                self.open_documents_for_server(&root, family, cx);
            }
            HostEvent::ServerState {
                root,
                family,
                state: xenon_lsp::ClientState::Failed,
            } => self.set_server_status(&root, family, Some("Language server stopped".into()), cx),
            HostEvent::ServerState { .. } => {}
        }
    }

    fn set_server_status(
        &self,
        root: &Path,
        family: LanguageFamily,
        status: Option<String>,
        cx: &mut Context<Self>,
    ) {
        for view in self.editors_under(root) {
            let path = view.read(cx).path().to_path_buf();
            if language_for_path(&path).is_some_and(|(candidate, _)| candidate == family) {
                let status = status.clone();
                view.update(cx, |editor, cx| editor.set_lsp_status(status, cx));
            }
        }
    }

    fn open_documents_for_server(
        &mut self,
        root: &Path,
        family: LanguageFamily,
        cx: &mut Context<Self>,
    ) {
        let editors = self.editors_under(root);
        for view in editors {
            let path = view.read(cx).path().to_path_buf();
            if language_for_path(&path).is_some_and(|(candidate, _)| candidate == family) {
                self.try_open_lsp_document(root, family, &view, cx);
            }
        }
    }

    fn try_open_lsp_document(
        &mut self,
        root: &Path,
        family: LanguageFamily,
        view: &Entity<EditorView>,
        cx: &App,
    ) {
        let Some((path, text)) = view.read(cx).text_snapshot() else {
            return;
        };
        if self
            .lsp
            .documents
            .get(&path)
            .is_some_and(|document| document.open)
        {
            return;
        }
        let Some((_, language_id)) = language_for_path(&path) else {
            return;
        };
        if self
            .lsp
            .host
            .open_document(
                root,
                family,
                DocumentSnapshot {
                    path: path.clone(),
                    language_id: language_id.into(),
                    text,
                },
            )
            .is_ok()
        {
            self.lsp.documents.insert(
                path,
                LspDocumentState {
                    open: true,
                    synced_version: Some(0),
                    ..Default::default()
                },
            );
        }
    }

    fn apply_highlights(
        &mut self,
        path: &Path,
        highlights: Vec<DocumentHighlight>,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.editor_for_path(path) else {
            return;
        };
        let ranges = highlights
            .into_iter()
            .filter_map(|highlight| {
                let start_col = self.utf16_col_for_path(
                    path,
                    highlight.start_line,
                    highlight.start_character_utf16,
                    cx,
                );
                let end_col = self.utf16_col_for_path(
                    path,
                    highlight.end_line,
                    highlight.end_character_utf16,
                    cx,
                );
                Some(
                    view.read(cx).char_offset(highlight.start_line, start_col)?
                        ..view.read(cx).char_offset(highlight.end_line, end_col)?,
                )
            })
            .collect();
        let diagnostics = self.editor_diagnostics(path, &view, cx);
        view.update(cx, |editor, cx| {
            editor.set_lsp_decorations(ranges, diagnostics, cx);
        });
    }

    fn apply_lsp_decorations(
        &mut self,
        path: &Path,
        occurrences: Vec<std::ops::Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.editor_for_path(path) else {
            return;
        };
        let diagnostics = self.editor_diagnostics(path, &view, cx);
        view.update(cx, |editor, cx| {
            editor.set_lsp_decorations(occurrences, diagnostics, cx);
        });
    }

    fn editor_diagnostics(
        &self,
        path: &Path,
        view: &Entity<EditorView>,
        cx: &App,
    ) -> Vec<EditorDiagnostic> {
        self.lsp
            .documents
            .get(path)
            .map(|document| {
                document
                    .diagnostics
                    .iter()
                    .filter_map(|diagnostic| {
                        let severity = match diagnostic.severity {
                            Some(DiagnosticSeverity::Error) => EditorDiagnosticSeverity::Error,
                            Some(DiagnosticSeverity::Warning) => EditorDiagnosticSeverity::Warning,
                            _ => return None,
                        };
                        let start_col = self.utf16_col_for_path(
                            path,
                            diagnostic.range.start.0,
                            diagnostic.range.start.1,
                            cx,
                        );
                        let end_col = self.utf16_col_for_path(
                            path,
                            diagnostic.range.end.0,
                            diagnostic.range.end.1,
                            cx,
                        );
                        Some(EditorDiagnostic {
                            range: view
                                .read(cx)
                                .char_offset(diagnostic.range.start.0, start_col)?
                                ..view.read(cx).char_offset(diagnostic.range.end.0, end_col)?,
                            severity,
                            message: diagnostic.message.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn lsp_context(&self, path: &Path) -> Option<(PathBuf, LanguageFamily)> {
        let (family, _) = language_for_path(path)?;
        let root = self
            .registry
            .workspaces
            .iter()
            .filter(|workspace| path.starts_with(&workspace.root))
            .max_by_key(|workspace| workspace.root.components().count())?
            .root
            .clone();
        Some((root, family))
    }

    fn editor_for_path(&self, path: &Path) -> Option<Entity<EditorView>> {
        self.contents.values().find_map(|content| {
            let root = content.root.as_ref()?;
            let (pane, index) = root.find_editor_path(path)?;
            root.find_leaf(pane)?.tabs.get(index)?.as_editor().cloned()
        })
    }

    fn editors_under(&self, root: &Path) -> Vec<Entity<EditorView>> {
        let mut editors = Vec::new();
        for content in self.contents.values() {
            if let Some(node) = content.root.as_ref() {
                collect_editors(node, root, &mut editors);
            }
        }
        editors
    }

    fn utf16_col_for_path(&self, path: &Path, row: u32, utf16: u32, cx: &App) -> u32 {
        self.editor_for_path(path)
            .and_then(|view| view.read(cx).line_text(row))
            .map_or(0, |line| utf16_to_char_col(&line, utf16) as u32)
    }

    fn bump_lsp_generation(&mut self, path: &Path) -> u64 {
        let document = self.lsp.documents.entry(path.to_path_buf()).or_default();
        let generation = document.request_generation.saturating_add(1);
        document.request_generation = generation;
        generation
    }

    fn bump_sync_generation(&mut self, path: &Path) -> u64 {
        let document = self.lsp.documents.entry(path.to_path_buf()).or_default();
        let generation = document.sync_generation.saturating_add(1);
        document.sync_generation = generation;
        generation
    }

    fn server_config(&self, family: LanguageFamily) -> Option<ServerConfig> {
        let configured = match family {
            LanguageFamily::Rust => &self.lsp.settings.rust,
            LanguageFamily::TypeScript => &self.lsp.settings.typescript,
        };
        if let Some(command) = configured.command.clone() {
            return Some(ServerConfig {
                command,
                args: configured.args.clone(),
            });
        }
        let command = match family {
            LanguageFamily::Rust => "rust-analyzer",
            LanguageFamily::TypeScript if executable_on_path("typescript-language-server") => {
                "typescript-language-server"
            }
            LanguageFamily::TypeScript if executable_on_path("vtsls") => "vtsls",
            LanguageFamily::TypeScript => return None,
        };
        Some(ServerConfig {
            command: command.into(),
            args: configured.args.clone(),
        })
    }
}

fn missing_server_message(family: LanguageFamily) -> String {
    match family {
        LanguageFamily::Rust => "rust-analyzer not found".into(),
        LanguageFamily::TypeScript => "TypeScript language server not found".into(),
    }
}
