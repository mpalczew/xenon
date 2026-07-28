use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;

use gpui::Task;
use xenon_lsp::{Diagnostic, HostEvent, LspHost};

#[derive(Default)]
pub(super) struct LspDocumentState {
    pub(super) open: bool,
    pub(super) synced_version: Option<i32>,
    pub(super) diagnostics_version: Option<i32>,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) request_generation: u64,
    pub(super) sync_generation: u64,
}

pub(crate) struct LspState {
    pub(super) host: LspHost,
    pub(super) settings: xenon_store::LspSettings,
    pub(super) documents: HashMap<PathBuf, LspDocumentState>,
    pub(super) event_task: Option<Task<()>>,
}

impl LspState {
    pub(crate) fn new(settings: xenon_store::LspSettings) -> (Self, mpsc::Receiver<HostEvent>) {
        let (host, events) = LspHost::new();
        (
            Self {
                host,
                settings,
                documents: HashMap::new(),
                event_task: None,
            },
            events,
        )
    }

    pub(crate) fn stop_workspace(&mut self, root: &Path) {
        self.host.stop_workspace(root);
        self.documents.retain(|path, _| !path.starts_with(root));
    }
}
