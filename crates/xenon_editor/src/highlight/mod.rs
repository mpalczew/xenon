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
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "boolean",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "embedded",
    "function",
    "function.builtin",
    "function.method",
    "function.macro",
    "keyword",
    "keyword.function",
    "keyword.operator",
    "keyword.return",
    "label",
    "module",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.escape",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    // Markdown (tree-sitter-md); mapped by `theme_key`.
    "text.title",
    "text.emphasis",
    "text.strong",
    "text.literal",
    "text.reference",
    "text.uri",
];

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
    let Ok(events) = highlighter.highlight(config, source.as_bytes(), None, |_| None) else {
        return Vec::new();
    };
    collect_spans!(events)
}

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
    fn highlights_typescript_and_tsx() {
        let ts = "const n: number = 1;\nfunction f(x: string): void {}\n";
        let spans = spans_for_path(Path::new("a.ts"), ts);
        assert!(
            spans
                .iter()
                .any(|s| s.name == "keyword" && &ts[s.start..s.end] == "const"),
            "expected TypeScript keyword highlight"
        );
        let tsx = "const el = <div className=\"x\">hi</div>;\n";
        assert!(!spans_for_path(Path::new("a.tsx"), tsx).is_empty());
    }

    #[test]
    fn highlights_common_languages() {
        let samples: &[(&str, &str)] = &[
            ("a.go", "package main\nfunc main() {}\n"),
            ("a.sh", "echo hello\n"),
            ("a.html", "<div id=\"x\"></div>\n"),
            ("a.css", "body { color: red; }\n"),
            ("a.yml", "key: value\n"),
            ("a.c", "int main(void) { return 0; }\n"),
            ("a.rb", "def f; end\n"),
            ("A.java", "class A { }\n"),
            ("a.lua", "local x = 1\n"),
            ("a.swift", "func f() {}\n"),
            ("a.scala", "object A { }\n"),
            ("a.ex", "defmodule A do\nend\n"),
            ("a.hs", "main = putStrLn \"hi\"\n"),
            ("a.php", "<?php echo 1;\n"),
            ("a.zig", "pub fn main() void {}\n"),
            ("a.diff", "--- a\n+++ b\n"),
            ("a.xml", "<root/>\n"),
            ("a.proto", "syntax = \"proto3\";\n"),
            ("a.dart", "void main() {}\n"),
            ("a.r", "x <- 1\n"),
            ("a.nix", "{ x = 1; }\n"),
            ("a.sol", "contract C {}\n"),
            ("a.ml", "let x = 1\n"),
            ("a.cs", "class A {}\n"),
            ("a.cmake", "project(x)\n"),
            ("a.glsl", "void main() {}\n"),
            ("a.ini", "[section]\nk=v\n"),
            ("a.scm", "(define x 1)\n"),
            ("a.ps1", "Write-Host hi\n"),
            ("a.elm", "module Main exposing (..)\n"),
            ("a.svelte", "<script>let x = 1</script>\n"),
        ];
        for &(path, src) in samples {
            assert!(
                !spans_for_path(Path::new(path), src).is_empty(),
                "no highlights for {path}"
            );
        }
        assert!(!spans_for_path(Path::new("Makefile"), "all:\n\techo hi\n").is_empty());
    }

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
    fn highlights_env_files() {
        // ini grammar: keys as property, `=` as operator (values uncolored).
        let source = "API_KEY=abc123\nDEBUG=true\n";
        for path in [
            "secrets.env",
            ".env",
            ".env.local",
            ".env.example",
            "homelab/secrets.env",
        ] {
            let spans = spans_for_path(Path::new(path), source);
            assert!(!spans.is_empty(), "no highlights for {path}");
            assert!(
                spans
                    .iter()
                    .any(|s| s.name == "property" && &source[s.start..s.end] == "API_KEY"),
                "expected property highlight for key in {path}"
            );
            assert!(
                spans
                    .iter()
                    .any(|s| s.name == "operator" && &source[s.start..s.end] == "="),
                "expected operator highlight in {path}"
            );
        }
    }

    #[test]
    fn unknown_extension_has_no_spans() {
        assert!(spans_for_path(Path::new("a.txt"), "plain text").is_empty());
    }

    #[test]
    fn spans_are_sorted_and_nonoverlapping() {
        let spans = spans_for_path(Path::new("a.rs"), "let x = \"hi\"; // c\nfn f() {}");
        for pair in spans.windows(2) {
            assert!(pair[0].end <= pair[1].start);
        }
    }
}
