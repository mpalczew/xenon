use std::ops::Range;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub line: u32,
    pub character_utf16: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightKind {
    Text,
    Read,
    Write,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub start_line: u32,
    pub start_character_utf16: u32,
    pub end_line: u32,
    pub end_character_utf16: u32,
    pub kind: HighlightKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range<(u32, u32)>,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
}
