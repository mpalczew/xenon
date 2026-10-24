//! Tree-sitter syntax highlighting. Produces byte-range spans tagged with a
//! capture name; the view maps each name to a theme color. Dispatches by file
//! extension (and a few special basenames) across bundled grammars.

mod grammars;
mod lang;

use std::path::Path;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use grammars::{MARKDOWN, config, injection_config};
use lang::{lang_for_basename, lang_for_ext, lang_for_name};

/// Capture names we color. Unlisted captures are ignored by `configure`.
/// Prefer more-specific names first when two recognized names share the same
/// part-set length — tree-sitter-highlight picks the longest part match.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "boolean",
    "comment",
    "comment.doc",
    "comment.documentation",
    "constant",
    "constant.builtin",
    "constructor",
    "diff.plus",
    "diff.minus",
    "embedded",
    "enum",
    "function",
    "function.builtin",
    "function.method",
    "function.macro",
    "function.call",
    "keyword",
    "keyword.function",
    "keyword.operator",
    "keyword.return",
    "keyword.import",
    "keyword.conditional",
    "keyword.repeat",
    "keyword.modifier",
    "keyword.type",
    "label",
    "module",
    "module.builtin",
    "namespace",
    "number",
    "operator",
    "preproc",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "punctuation.list_marker",
    "selector",
    "selector.pseudo",
    "string",
    "string.escape",
    "string.regex",
    "string.regexp",
    "string.special",
    "string.special.symbol",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "variable.member",
    "variable.special",
    "variant",
    "text.title",
    "text.emphasis",
    "text.strong",
    "text.literal",
    "text.reference",
    "text.uri",
    "text.strike",
    "markup.heading",
    "markup.list",
    "markup.link",
    "markup.raw",
    "markup.italic",
    "markup.bold",
    "markup.strikethrough",
];

fn theme_key(name: &'static str) -> &'static str {
    match name {
        "text.title" | "markup.heading" => "title",
        "text.emphasis" | "markup.italic" => "emphasis",
        "text.strong" | "markup.bold" => "emphasis.strong",
        "text.reference" | "markup.link" => "link_text",
        "text.uri" => "link_uri",
        "text.literal" | "markup.raw" => "text.literal",
        "text.strike" | "markup.strikethrough" => "comment",
        "comment.documentation" => "comment.doc",
        "string.regexp" => "string.regex",
        "module.builtin" => "module",
        "namespace" => "namespace",
        "variable.member" => "property",
        "function.call" => "function",
        "keyword.import"
        | "keyword.conditional"
        | "keyword.repeat"
        | "keyword.modifier"
        | "keyword.type" => "keyword",
        "markup.list" => "punctuation.list_marker",
        other => other,
    }
}

/// A highlighted byte range `[start, end)` tagged with its capture name.
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub name: &'static str,
}

/// Highlight spans for a file's source, chosen by extension or basename.
pub fn spans_for_path(path: &Path, source: &str) -> Vec<Span> {
    let ext = path.extension().and_then(|e| e.to_str());
    if matches!(ext, Some("md" | "markdown" | "mdx")) {
        return highlight_markdown(source);
    }
    let lang = ext.and_then(lang_for_ext).or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .and_then(lang_for_basename)
    });
    let Some(config) = lang.and_then(config) else {
        return Vec::new();
    };
    highlight(config, source)
}

/// Highlight spans for a named language (markdown fences).
pub fn spans_for_lang(name: &str, source: &str) -> Vec<Span> {
    let Some(config) = lang_for_name(name).and_then(config) else {
        return Vec::new();
    };
    highlight(config, source)
}

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
    #[allow(clippy::redundant_closure)]
    let Ok(events) = highlighter.highlight(config, source.as_bytes(), None, |name| {
        injection_config(name)
    }) else {
        return Vec::new();
    };
    collect_spans!(events)
}

fn highlight_markdown(source: &str) -> Vec<Span> {
    let Some(config) = MARKDOWN.as_ref() else {
        return Vec::new();
    };
    highlight(config, source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn highlights_markdown_title_and_fenced_code() {
        let source = "# Title\n\nsome **bold** text\n\n```rust\nfn f() {}\n```\n";
        let spans = spans_for_path(Path::new("readme.md"), source);
        assert!(spans.iter().any(|s| s.name == "title"));
        assert!(
            spans
                .iter()
                .any(|s| s.name == "keyword" && &source[s.start..s.end] == "fn")
        );
    }

    #[test]
    fn highlights_html_script_injection() {
        let source = "<script>const x = 1;</script>\n";
        let spans = spans_for_path(Path::new("a.html"), source);
        assert!(
            spans
                .iter()
                .any(|s| s.name == "keyword" && &source[s.start..s.end] == "const"),
            "expected JS keyword via HTML injection, got {:?}",
            spans
                .iter()
                .map(|s| (s.name, &source[s.start..s.end]))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn highlights_js_regex_injection() {
        let source = "const re = /ab+c/;\n";
        let spans = spans_for_path(Path::new("a.js"), source);
        let names: Vec<_> = spans
            .iter()
            .map(|s| (s.name, &source[s.start..s.end]))
            .collect();
        assert!(
            spans
                .iter()
                .any(|s| s.name == "string" && &source[s.start..s.end] == "a"),
            "expected regex pattern highlight via injection, got {names:?}"
        );
        assert!(
            spans
                .iter()
                .any(|s| s.name == "operator" && &source[s.start..s.end] == "+"),
            "expected regex quantifier highlight, got {names:?}"
        );
    }

    #[test]
    fn spans_are_sorted_and_nonoverlapping() {
        let spans = spans_for_path(Path::new("a.rs"), "let x = \"hi\"; // c\nfn f() {}");
        for pair in spans.windows(2) {
            assert!(pair[0].end <= pair[1].start);
        }
    }
}
