use std::path::Path;

use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    VersionedTextDocumentIdentifier,
};
use serde_json::json;

use super::{DocumentSnapshot, LanguageFamily, LspHost, file_uri, lock_error, to_value};
use crate::LspError;

impl LspHost {
    pub fn open_document(
        &self,
        root: &Path,
        family: LanguageFamily,
        document: DocumentSnapshot,
    ) -> Result<(), LspError> {
        let path = document.path;
        let uri = file_uri(&path)?;
        self.documents
            .lock()
            .map_err(lock_error)?
            .insert(path.clone(), 0);
        self.client(root, family)?.notify(
            "textDocument/didOpen",
            to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: document.language_id,
                    version: 0,
                    text: document.text,
                },
            })?,
        )
    }

    pub fn change_document(
        &self,
        root: &Path,
        family: LanguageFamily,
        path: &Path,
        text: String,
    ) -> Result<i32, LspError> {
        let version = {
            let mut documents = self.documents.lock().map_err(lock_error)?;
            let current = documents.entry(path.to_path_buf()).or_insert(0);
            *current = current.saturating_add(1);
            *current
        };
        self.client(root, family)?.notify(
            "textDocument/didChange",
            to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: file_uri(path)?,
                    version,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text,
                }],
            })?,
        )?;
        Ok(version)
    }

    pub fn save_document(
        &self,
        root: &Path,
        family: LanguageFamily,
        path: &Path,
    ) -> Result<(), LspError> {
        self.client(root, family)?.notify(
            "textDocument/didSave",
            to_value(DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: file_uri(path)?,
                },
                text: None,
            })?,
        )
    }

    pub fn close_document(
        &self,
        root: &Path,
        family: LanguageFamily,
        path: &Path,
    ) -> Result<(), LspError> {
        self.documents.lock().map_err(lock_error)?.remove(path);
        self.client(root, family)?.notify(
            "textDocument/didClose",
            json!({"textDocument":{"uri":file_uri(path)?}}),
        )
    }
}
