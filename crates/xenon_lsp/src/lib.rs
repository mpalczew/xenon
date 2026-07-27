//! Small, runtime-neutral LSP client used by Xenon's editor integration.

mod client;
mod framing;
mod host;
mod position;
mod types;

pub use client::{ClientState, Request};
pub use framing::{read_message, write_message};
pub use host::{DocumentSnapshot, HostEvent, LanguageFamily, LspHost, ServerConfig};
pub use position::{char_col_to_utf16, utf16_to_char_col};
pub use types::{Diagnostic, DiagnosticSeverity, DocumentHighlight, HighlightKind, Location};

#[derive(Debug, thiserror::Error, Clone)]
pub enum LspError {
    #[error("language server is not available")]
    NotAvailable,
    #[error("language server request timed out")]
    Timeout,
    #[error("language server RPC error: {0}")]
    Rpc(String),
    #[error("language server I/O error: {0}")]
    Io(String),
    #[error("language server returned no result")]
    NoResult,
    #[error("language server protocol error: {0}")]
    Protocol(String),
}

impl From<std::io::Error> for LspError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}
