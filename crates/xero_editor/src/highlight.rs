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
    // Markdown (tree-sitter-md capture names); mapped to theme keys by `theme_key`.
    "text.title",
    "text.emphasis",
    "text.strong",
    "text.literal",
    "text.reference",
    "text.uri",
    "string.escape",
    "punctuation.special",
];

/// Map a grammar capture name to the theme's syntax key where they differ
/// (markdown's `text.*` names vs. the One theme's `title`/`emphasis`/`link_*`).
fn theme_key(name: &'static str) -> &'static str {
    match name {
        "text.title" => "title",
        "text.emphasis" => "emphasis",
        "text.strong" => "emphasis.strong",
        "text.reference" => "link_text",
        "text.uri" => "link_uri",
        other => other,
    }
}

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
    build_injected(language, "source", query, "")
}

/// Build a config that may inject other grammars (used for markdown's inline and
/// fenced-code-block injections).
fn build_injected(
    language: tree_sitter::Language,
    name: &str,
    highlights: &str,
    injections: &str,
) -> Option<HighlightConfiguration> {
    let mut config =
        HighlightConfiguration::new(language, name, highlights, injections, "").ok()?;
    config.configure(HIGHLIGHT_NAMES);
    Some(config)
}

static MARKDOWN: LazyLock<Option<HighlightConfiguration>> = LazyLock::new(|| {
    build_injected(
        Language::new(tree_sitter_md::LANGUAGE),
        "markdown",
        tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
        tree_sitter_md::INJECTION_QUERY_BLOCK,
    )
});
static MARKDOWN_INLINE: LazyLock<Option<HighlightConfiguration>> = LazyLock::new(|| {
    build_injected(
        Language::new(tree_sitter_md::INLINE_LANGUAGE),
        "markdown_inline",
        tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
        tree_sitter_md::INJECTION_QUERY_INLINE,
    )
});

/// Resolve a markdown injection: the inline grammar, or a fenced-code language.
fn injection_config(name: &str) -> Option<&'static HighlightConfiguration> {
    match name {
        "markdown_inline" => MARKDOWN_INLINE.as_ref(),
        other => lang_for_name(other).and_then(config),
    }
}

macro_rules! config {
    ($name:ident, $lang:expr, $query:expr) => {
        static $name: LazyLock<Option<HighlightConfiguration>> =
            LazyLock::new(|| build(Language::new($lang), $query));
    };
}

config!(
    RUST,
    tree_sitter_rust::LANGUAGE,
    tree_sitter_rust::HIGHLIGHTS_QUERY
);
config!(
    JSON,
    tree_sitter_json::LANGUAGE,
    tree_sitter_json::HIGHLIGHTS_QUERY
);
config!(
    TOML,
    tree_sitter_toml_ng::LANGUAGE,
    tree_sitter_toml_ng::HIGHLIGHTS_QUERY
);
config!(
    PYTHON,
    tree_sitter_python::LANGUAGE,
    tree_sitter_python::HIGHLIGHTS_QUERY
);
config!(
    JAVASCRIPT,
    tree_sitter_javascript::LANGUAGE,
    tree_sitter_javascript::HIGHLIGHT_QUERY
);

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
    let ext = path.extension().and_then(|e| e.to_str());
    if matches!(ext, Some("md" | "markdown" | "mdx")) {
        return highlight_markdown(source);
    }
    let Some(config) = ext.and_then(lang_for_ext).and_then(config) else {
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

/// Drain a highlight event stream into spans. A macro (not a function) so the
/// injection closure's lifetime infers at each call site — the stream borrows
/// the local `Highlighter`, which can't be named across a function boundary.
macro_rules! collect_spans {
    ($events:expr) => {{
        let mut spans = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        for event in $events {
            match event {
                Ok(HighlightEvent::HighlightStart(highlight)) => stack.push(highlight.0),
                Ok(HighlightEvent::HighlightEnd) => {
                    stack.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    if let Some(&index) = stack.last() {
                        spans.push(Span {
                            start,
                            end,
                            name: theme_key(HIGHLIGHT_NAMES[index]),
                        });
                    }
                }
                Err(_) => break,
            }
        }
        spans
    }};
}

fn highlight(config: &HighlightConfiguration, source: &str) -> Vec<Span> {
    let mut highlighter = Highlighter::new();
    let Ok(events) = highlighter.highlight(config, source.as_bytes(), None, |_| None) else {
        return Vec::new();
    };
    collect_spans!(events)
}

/// Highlight markdown, injecting its inline grammar and fenced-code languages.
fn highlight_markdown(source: &str) -> Vec<Span> {
    let Some(config) = MARKDOWN.as_ref() else {
        return Vec::new();
    };
    let mut highlighter = Highlighter::new();
    #[allow(clippy::redundant_closure)]
    let Ok(events) = highlighter.highlight(config, source.as_bytes(), None, |name| {
        injection_config(name)
    }) else {
        return Vec::new();
    };
    collect_spans!(events)
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
    fn highlights_markdown_title_and_fenced_code() {
        let source = "# Title\n\nsome **bold** text\n\n```rust\nfn f() {}\n```\n";
        let spans = spans_for_path(Path::new("readme.md"), source);
        // The heading maps to the theme's `title` key...
        assert!(spans.iter().any(|s| s.name == "title"));
        // ...and the fenced Rust block is highlighted via injection.
        assert!(
            spans
                .iter()
                .any(|s| s.name == "keyword" && &source[s.start..s.end] == "fn")
        );
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
