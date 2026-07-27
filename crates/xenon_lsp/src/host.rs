use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use lsp_types::{
    ClientCapabilities, Diagnostic as LspDiagnostic, DiagnosticSeverity as LspSeverity,
    DocumentHighlight as LspHighlight, DocumentHighlightKind, DocumentHighlightParams,
    GotoDefinitionParams, GotoDefinitionResponse, InitializeParams, InitializeResult,
    InitializedParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
    WorkDoneProgressParams, WorkspaceFolder,
};
use serde_json::Value;
use url::Url;

use crate::LspError;
use crate::client::{Client, ClientEvent, ClientState, Request};
use crate::types::{Diagnostic, DiagnosticSeverity, DocumentHighlight, HighlightKind, Location};

mod documents;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum LanguageFamily {
    Rust,
    TypeScript,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

pub struct DocumentSnapshot {
    pub path: PathBuf,
    pub language_id: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub enum HostEvent {
    Diagnostics {
        path: PathBuf,
        version: Option<i32>,
        diagnostics: Vec<Diagnostic>,
    },
    ServerState {
        root: PathBuf,
        family: LanguageFamily,
        state: ClientState,
    },
}

struct Server {
    client: Arc<Client>,
    capabilities: Arc<Mutex<Option<InitializeResult>>>,
}

#[derive(Clone)]
pub struct LspHost {
    servers: Arc<Mutex<HashMap<(PathBuf, LanguageFamily), Server>>>,
    documents: Arc<Mutex<HashMap<PathBuf, i32>>>,
    event_tx: mpsc::Sender<HostEvent>,
}

impl LspHost {
    pub fn new() -> (Self, mpsc::Receiver<HostEvent>) {
        let (event_tx, event_rx) = mpsc::channel();
        (
            Self {
                servers: Arc::default(),
                documents: Arc::default(),
                event_tx,
            },
            event_rx,
        )
    }

    pub fn start_server(
        &self,
        root: PathBuf,
        family: LanguageFamily,
        config: ServerConfig,
    ) -> Result<(), LspError> {
        let key = (root.clone(), family);
        if self.servers.lock().map_err(lock_error)?.contains_key(&key) {
            return Ok(());
        }
        let (client_tx, client_rx) = mpsc::channel();
        let client = Arc::new(Client::spawn(&config.command, &config.args, client_tx)?);
        let capabilities = Arc::new(Mutex::new(None));
        self.servers.lock().map_err(lock_error)?.insert(
            key,
            Server {
                client: Arc::clone(&client),
                capabilities: Arc::clone(&capabilities),
            },
        );
        relay_events(root.clone(), family, client_rx, self.event_tx.clone());
        initialize(client, capabilities, root, family, self.event_tx.clone());
        Ok(())
    }

    pub fn stop_workspace(&self, root: &Path) {
        let removed = if let Ok(mut servers) = self.servers.lock() {
            let keys = servers
                .keys()
                .filter(|(candidate, _)| candidate == root)
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| servers.remove(&key))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        for server in removed {
            server.client.shutdown();
        }
    }

    pub fn definition(
        &self,
        root: &Path,
        family: LanguageFamily,
        path: &Path,
        position: Position,
    ) -> Result<Request, LspError> {
        if !self.supports(root, family, Feature::Definition)? {
            return Err(LspError::NotAvailable);
        }
        self.client(root, family)?.request(
            "textDocument/definition",
            to_value(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: file_uri(path)?,
                    },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            })?,
        )
    }

    pub fn document_highlight(
        &self,
        root: &Path,
        family: LanguageFamily,
        path: &Path,
        position: Position,
    ) -> Result<Request, LspError> {
        if !self.supports(root, family, Feature::DocumentHighlight)? {
            return Err(LspError::NotAvailable);
        }
        self.client(root, family)?.request(
            "textDocument/documentHighlight",
            to_value(DocumentHighlightParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: file_uri(path)?,
                    },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            })?,
        )
    }

    pub fn decode_definition(value: Value) -> Result<Vec<Location>, LspError> {
        let response: Option<GotoDefinitionResponse> =
            serde_json::from_value(value).map_err(|error| LspError::Protocol(error.to_string()))?;
        let locations = match response {
            None => Vec::new(),
            Some(GotoDefinitionResponse::Scalar(location)) => vec![location],
            Some(GotoDefinitionResponse::Array(locations)) => locations,
            Some(GotoDefinitionResponse::Link(links)) => {
                return links
                    .into_iter()
                    .map(|link| {
                        location_from_parts(link.target_uri, link.target_selection_range.start)
                    })
                    .collect();
            }
        };
        locations
            .into_iter()
            .map(|location| location_from_parts(location.uri, location.range.start))
            .collect()
    }

    pub fn decode_highlights(value: Value) -> Result<Vec<DocumentHighlight>, LspError> {
        let highlights: Option<Vec<LspHighlight>> =
            serde_json::from_value(value).map_err(|error| LspError::Protocol(error.to_string()))?;
        Ok(highlights
            .unwrap_or_default()
            .into_iter()
            .map(|highlight| DocumentHighlight {
                start_line: highlight.range.start.line,
                start_character_utf16: highlight.range.start.character,
                end_line: highlight.range.end.line,
                end_character_utf16: highlight.range.end.character,
                kind: match highlight.kind {
                    Some(DocumentHighlightKind::READ) => HighlightKind::Read,
                    Some(DocumentHighlightKind::WRITE) => HighlightKind::Write,
                    _ => HighlightKind::Text,
                },
            })
            .collect())
    }

    fn client(&self, root: &Path, family: LanguageFamily) -> Result<Arc<Client>, LspError> {
        let servers = self.servers.lock().map_err(lock_error)?;
        let server = servers
            .get(&(root.to_path_buf(), family))
            .ok_or(LspError::NotAvailable)?;
        if server.client.state() != ClientState::Running {
            return Err(LspError::NotAvailable);
        }
        Ok(Arc::clone(&server.client))
    }

    fn supports(
        &self,
        root: &Path,
        family: LanguageFamily,
        feature: Feature,
    ) -> Result<bool, LspError> {
        let servers = self.servers.lock().map_err(lock_error)?;
        let server = servers
            .get(&(root.to_path_buf(), family))
            .ok_or(LspError::NotAvailable)?;
        let capabilities = server.capabilities.lock().map_err(lock_error)?;
        let Some(result) = capabilities.as_ref() else {
            return Ok(false);
        };
        Ok(match feature {
            Feature::Definition => provider_enabled(&result.capabilities.definition_provider),
            Feature::DocumentHighlight => {
                provider_enabled(&result.capabilities.document_highlight_provider)
            }
        })
    }
}

fn provider_enabled<T>(provider: &Option<lsp_types::OneOf<bool, T>>) -> bool {
    match provider {
        Some(lsp_types::OneOf::Left(enabled)) => *enabled,
        Some(lsp_types::OneOf::Right(_)) => true,
        None => false,
    }
}

enum Feature {
    Definition,
    DocumentHighlight,
}

fn initialize(
    client: Arc<Client>,
    capabilities: Arc<Mutex<Option<InitializeResult>>>,
    root: PathBuf,
    family: LanguageFamily,
    event_tx: mpsc::Sender<HostEvent>,
) {
    thread::spawn(move || {
        let result = (|| {
            let params = InitializeParams {
                process_id: Some(std::process::id()),
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: file_uri(&root)?,
                    name: root
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("workspace")
                        .to_string(),
                }]),
                capabilities: ClientCapabilities {
                    general: Some(lsp_types::GeneralClientCapabilities {
                        position_encodings: Some(vec![lsp_types::PositionEncodingKind::UTF16]),
                        ..Default::default()
                    }),
                    workspace: Some(lsp_types::WorkspaceClientCapabilities {
                        configuration: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                client_info: Some(lsp_types::ClientInfo {
                    name: "Xenon".into(),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                }),
                ..Default::default()
            };
            let request = client.request("initialize", to_value(params)?)?;
            let value = request.recv()?;
            let initialized: InitializeResult = serde_json::from_value(value)
                .map_err(|error| LspError::Protocol(error.to_string()))?;
            if initialized
                .capabilities
                .position_encoding
                .as_ref()
                .is_some_and(|encoding| encoding != &lsp_types::PositionEncodingKind::UTF16)
            {
                return Err(LspError::Protocol(
                    "server selected an unsupported position encoding".into(),
                ));
            }
            *capabilities.lock().map_err(lock_error)? = Some(initialized);
            client.set_running();
            client.notify("initialized", to_value(InitializedParams {})?)?;
            Ok::<(), LspError>(())
        })();
        let state = if result.is_ok() {
            ClientState::Running
        } else {
            ClientState::Failed
        };
        let _ = event_tx.send(HostEvent::ServerState {
            root,
            family,
            state,
        });
    });
}

fn relay_events(
    root: PathBuf,
    family: LanguageFamily,
    rx: mpsc::Receiver<ClientEvent>,
    event_tx: mpsc::Sender<HostEvent>,
) {
    thread::spawn(move || {
        for event in rx {
            match event {
                ClientEvent::Notification { method, params }
                    if method == "textDocument/publishDiagnostics" =>
                {
                    if let Some(event) = decode_diagnostics(params) {
                        let _ = event_tx.send(event);
                    }
                }
                ClientEvent::StateChanged(state) => {
                    let _ = event_tx.send(HostEvent::ServerState {
                        root: root.clone(),
                        family,
                        state,
                    });
                }
                ClientEvent::Notification { .. } => {}
            }
        }
    });
}

fn decode_diagnostics(params: Value) -> Option<HostEvent> {
    let params: lsp_types::PublishDiagnosticsParams = serde_json::from_value(params).ok()?;
    let path = Url::parse(params.uri.as_str()).ok()?.to_file_path().ok()?;
    let diagnostics = params
        .diagnostics
        .into_iter()
        .map(convert_diagnostic)
        .collect();
    Some(HostEvent::Diagnostics {
        path,
        version: params.version,
        diagnostics,
    })
}

fn convert_diagnostic(diagnostic: LspDiagnostic) -> Diagnostic {
    Diagnostic {
        range: (
            diagnostic.range.start.line,
            diagnostic.range.start.character,
        )..(diagnostic.range.end.line, diagnostic.range.end.character),
        severity: diagnostic.severity.map(|severity| match severity {
            LspSeverity::ERROR => DiagnosticSeverity::Error,
            LspSeverity::WARNING => DiagnosticSeverity::Warning,
            LspSeverity::INFORMATION => DiagnosticSeverity::Information,
            _ => DiagnosticSeverity::Hint,
        }),
        message: diagnostic.message,
        source: diagnostic.source,
        code: diagnostic.code.map(|code| match code {
            lsp_types::NumberOrString::Number(number) => number.to_string(),
            lsp_types::NumberOrString::String(text) => text,
        }),
    }
}

fn location_from_parts(uri: Uri, position: Position) -> Result<Location, LspError> {
    let url = Url::parse(uri.as_str()).map_err(|error| LspError::Protocol(error.to_string()))?;
    Ok(Location {
        path: url
            .to_file_path()
            .map_err(|()| LspError::Protocol("definition URI is not a file".into()))?,
        line: position.line,
        character_utf16: position.character,
    })
}

fn file_uri(path: &Path) -> Result<Uri, LspError> {
    let url = Url::from_file_path(path)
        .map_err(|()| LspError::Protocol(format!("cannot convert {} to URI", path.display())))?;
    url.as_str()
        .parse::<Uri>()
        .map_err(|error| LspError::Protocol(error.to_string()))
}

fn to_value(value: impl serde::Serialize) -> Result<Value, LspError> {
    serde_json::to_value(value).map_err(|error| LspError::Protocol(error.to_string()))
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> LspError {
    LspError::Io("LSP state lock poisoned".into())
}

impl Drop for LspHost {
    fn drop(&mut self) {
        if Arc::strong_count(&self.servers) != 1 {
            return;
        }
        let servers = self
            .servers
            .lock()
            .map(|mut servers| {
                servers
                    .drain()
                    .map(|(_, server)| server)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for server in servers {
            server.client.shutdown();
        }
    }
}
