//! Tree-sitter syntax highlighting. Produces byte-range spans tagged with a
//! capture name; the view maps each name to a theme color. Dispatches by file
//! extension across the bundled grammars.

use std::path::Path;
use std::sync::LazyLock;

use tree_sitter::Language;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Capture names we color, chosen to overlap the grammars' highlight queries
/// and the theme's syntax palette (`SyntaxTheme::style_for_name`).
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "function",
    "function.method",
    "function.macro",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "string.special",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "tag",
];

/// A highlighted byte range `[start, end)` tagged with its capture name.
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub name: &'static str,
}

#[derive(Clone, Copy)]
enum Lang {
    Rust,
    Json,
    Toml,
    Python,
    JavaScript,
}

fn lang_for_ext(ext: &str) -> Option<Lang> {
    Some(match ext {
        "rs" => Lang::Rust,
        "json" => Lang::Json,
        "toml" => Lang::Toml,
        "py" | "pyi" => Lang::Python,
        "js" | "jsx" | "mjs" | "cjs" => Lang::JavaScript,
        _ => return None,
    })
}

/// Language named by a markdown fenced-code-block info string (```rust).
fn lang_for_name(name: &str) -> Option<Lang> {
    Some(match name {
        "rust" | "rs" => Lang::Rust,
        "json" => Lang::Json,
        "toml" => Lang::Toml,
        "python" | "py" => Lang::Python,
        "javascript" | "js" | "jsx" => Lang::JavaScript,
        _ => return None,
    })
}

fn build(language: tree_sitter::Language, query: &str) -> Option<HighlightConfiguration> {
    let mut config = HighlightConfiguration::new(language, "source", query, "", "").ok()?;
    config.configure(HIGHLIGHT_NAMES);
    Some(config)
}

macro_rules! config {
    ($name:ident, $lang:expr, $query:expr) => {
        static $name: LazyLock<Option<HighlightConfiguration>> =
            LazyLock::new(|| build(Language::new($lang), $query));
    };
}

config!(RUST, tree_sitter_rust::LANGUAGE, tree_sitter_rust::HIGHLIGHTS_QUERY);
config!(JSON, tree_sitter_json::LANGUAGE, tree_sitter_json::HIGHLIGHTS_QUERY);
config!(TOML, tree_sitter_toml_ng::LANGUAGE, tree_sitter_toml_ng::HIGHLIGHTS_QUERY);
config!(PYTHON, tree_sitter_python::LANGUAGE, tree_sitter_python::HIGHLIGHTS_QUERY);
config!(JAVASCRIPT, tree_sitter_javascript::LANGUAGE, tree_sitter_javascript::HIGHLIGHT_QUERY);

fn config(lang: Lang) -> Option<&'static HighlightConfiguration> {
    match lang {
        Lang::Rust => RUST.as_ref(),
        Lang::Json => JSON.as_ref(),
        Lang::Toml => TOML.as_ref(),
        Lang::Python => PYTHON.as_ref(),
        Lang::JavaScript => JAVASCRIPT.as_ref(),
    }
}

/// Highlight spans for a file's source, chosen by extension. Empty for
/// unsupported languages or on any highlighter error.
pub fn spans_for_path(path: &Path, source: &str) -> Vec<Span> {
    let Some(config) = path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(lang_for_ext)
        .and_then(config)
    else {
        return Vec::new();
    };
    highlight(config, source)
}

/// Highlight spans for source in a named language (markdown code fences). Empty
/// for unsupported languages.
pub fn spans_for_lang(name: &str, source: &str) -> Vec<Span> {
    let Some(config) = lang_for_name(name).and_then(config) else {
        return Vec::new();
    };
    highlight(config, source)
}

fn highlight(config: &HighlightConfiguration, source: &str) -> Vec<Span> {
    let mut highlighter = Highlighter::new();
    let Ok(events) = highlighter.highlight(config, source.as_bytes(), None, |_| None) else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for event in events {
        match event {
            Ok(HighlightEvent::HighlightStart(highlight)) => stack.push(highlight.0),
            Ok(HighlightEvent::HighlightEnd) => {
                stack.pop();
            }
            Ok(HighlightEvent::Source { start, end }) => {
                if let Some(&index) = stack.last() {
                    spans.push(Span { start, end, name: HIGHLIGHT_NAMES[index] });
                }
            }
            Err(_) => break,
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn highlights_a_rust_keyword() {
        let source = "fn main() {}";
        let spans = spans_for_path(Path::new("a.rs"), source);
        let kw = spans.iter().find(|s| &source[s.start..s.end] == "fn");
        assert_eq!(kw.map(|s| s.name), Some("keyword"));
    }

    #[test]
    fn highlights_toml_and_json_and_python() {
        assert!(!spans_for_path(Path::new("Cargo.toml"), "[package]\nname = \"x\"\n").is_empty());
        assert!(!spans_for_path(Path::new("a.json"), "{\"k\": 1}").is_empty());
        assert!(!spans_for_path(Path::new("a.py"), "def f():\n    return 1\n").is_empty());
    }

    #[test]
    fn unknown_extension_has_no_spans() {
        assert!(spans_for_path(Path::new("a.txt"), "plain text").is_empty());
        assert!(spans_for_path(Path::new("Makefile"), "all:\n").is_empty());
    }

    #[test]
    fn spans_are_sorted_and_nonoverlapping() {
        let spans = spans_for_path(Path::new("a.rs"), "let x = \"hi\"; // c\nfn f() {}");
        for pair in spans.windows(2) {
            assert!(pair[0].end <= pair[1].start);
        }
    }
}
