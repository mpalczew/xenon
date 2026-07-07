//! Tree-sitter syntax highlighting. Produces byte-range spans tagged with a
//! capture name; the view maps each name to a theme color. Rust only for now.

use std::sync::LazyLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Capture names we color, chosen to overlap both the Rust highlights query and
/// the theme's syntax palette (`SyntaxTheme::style_for_name`).
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
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
];

/// A highlighted byte range `[start, end)` tagged with its capture name.
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub name: &'static str,
}

static RUST_CONFIG: LazyLock<HighlightConfiguration> = LazyLock::new(|| {
    let language = tree_sitter::Language::new(tree_sitter_rust::LANGUAGE);
    let mut config = HighlightConfiguration::new(
        language,
        "rust",
        tree_sitter_rust::HIGHLIGHTS_QUERY,
        "",
        "",
    )
    .expect("rust highlights query parses");
    config.configure(HIGHLIGHT_NAMES);
    config
});

/// Highlight spans for a Rust source string, innermost capture per byte, in
/// source order. Returns empty on any highlighter error.
pub fn rust_spans(source: &str) -> Vec<Span> {
    let mut highlighter = Highlighter::new();
    let Ok(events) = highlighter.highlight(&RUST_CONFIG, source.as_bytes(), None, |_| None) else {
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

/// Whether a path is a language we can highlight (Rust only in v1).
pub fn is_highlightable(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("rs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_a_keyword() {
        let source = "fn main() {}";
        let spans = rust_spans(source);
        assert!(!spans.is_empty());
        // "fn" is a keyword at bytes 0..2.
        let kw = spans.iter().find(|s| &source[s.start..s.end] == "fn");
        assert_eq!(kw.map(|s| s.name), Some("keyword"));
    }

    #[test]
    fn spans_are_sorted_and_nonoverlapping() {
        let spans = rust_spans("let x = \"hi\"; // c\nfn f() {}");
        for pair in spans.windows(2) {
            assert!(pair[0].end <= pair[1].start, "spans must not overlap");
        }
    }

    #[test]
    fn only_rust_is_highlightable() {
        use std::path::Path;
        assert!(is_highlightable(Path::new("a/b.rs")));
        assert!(!is_highlightable(Path::new("a/b.txt")));
        assert!(!is_highlightable(Path::new("Makefile")));
    }
}
