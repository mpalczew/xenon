//! Lazy highlight configurations for each bundled grammar.

use std::sync::LazyLock;

use tree_sitter::Language;
use tree_sitter_highlight::HighlightConfiguration;

use super::HIGHLIGHT_NAMES;
use super::lang::{Lang, lang_for_name};

fn build(language: tree_sitter::Language, query: &str) -> Option<HighlightConfiguration> {
    build_injected(language, "source", query, "")
}

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

pub(super) static MARKDOWN: LazyLock<Option<HighlightConfiguration>> = LazyLock::new(|| {
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

pub(super) fn injection_config(name: &str) -> Option<&'static HighlightConfiguration> {
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
// TS/TSX: JS query + TS-specific deltas (upstream TS query is type-only).
static TYPESCRIPT: LazyLock<Option<HighlightConfiguration>> = LazyLock::new(|| {
    let query = format!(
        "{}\n{}",
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_typescript::HIGHLIGHTS_QUERY
    );
    build(
        Language::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
        &query,
    )
});
static TSX: LazyLock<Option<HighlightConfiguration>> = LazyLock::new(|| {
    let query = format!(
        "{}\n{}",
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_typescript::HIGHLIGHTS_QUERY
    );
    build(Language::new(tree_sitter_typescript::LANGUAGE_TSX), &query)
});
config!(
    GO,
    tree_sitter_go::LANGUAGE,
    tree_sitter_go::HIGHLIGHTS_QUERY
);
config!(
    BASH,
    tree_sitter_bash::LANGUAGE,
    tree_sitter_bash::HIGHLIGHT_QUERY
);
config!(
    HTML,
    tree_sitter_html::LANGUAGE,
    tree_sitter_html::HIGHLIGHTS_QUERY
);
config!(
    CSS,
    tree_sitter_css::LANGUAGE,
    tree_sitter_css::HIGHLIGHTS_QUERY
);
config!(
    YAML,
    tree_sitter_yaml::LANGUAGE,
    tree_sitter_yaml::HIGHLIGHTS_QUERY
);
config!(C, tree_sitter_c::LANGUAGE, tree_sitter_c::HIGHLIGHT_QUERY);
config!(
    CPP,
    tree_sitter_cpp::LANGUAGE,
    tree_sitter_cpp::HIGHLIGHT_QUERY
);
config!(
    RUBY,
    tree_sitter_ruby::LANGUAGE,
    tree_sitter_ruby::HIGHLIGHTS_QUERY
);
config!(
    JAVA,
    tree_sitter_java::LANGUAGE,
    tree_sitter_java::HIGHLIGHTS_QUERY
);
config!(
    LUA,
    tree_sitter_lua::LANGUAGE,
    tree_sitter_lua::HIGHLIGHTS_QUERY
);
config!(
    SWIFT,
    tree_sitter_swift::LANGUAGE,
    tree_sitter_swift::HIGHLIGHTS_QUERY
);
config!(
    SCALA,
    tree_sitter_scala::LANGUAGE,
    tree_sitter_scala::HIGHLIGHTS_QUERY
);
config!(
    ELIXIR,
    tree_sitter_elixir::LANGUAGE,
    tree_sitter_elixir::HIGHLIGHTS_QUERY
);
config!(
    HASKELL,
    tree_sitter_haskell::LANGUAGE,
    tree_sitter_haskell::HIGHLIGHTS_QUERY
);
config!(
    PHP,
    tree_sitter_php::LANGUAGE_PHP,
    tree_sitter_php::HIGHLIGHTS_QUERY
);
config!(
    ZIG,
    tree_sitter_zig::LANGUAGE,
    tree_sitter_zig::HIGHLIGHTS_QUERY
);
config!(
    MAKE,
    tree_sitter_make::LANGUAGE,
    tree_sitter_make::HIGHLIGHTS_QUERY
);
config!(
    DIFF,
    tree_sitter_diff::LANGUAGE,
    tree_sitter_diff::HIGHLIGHTS_QUERY
);
config!(
    XML,
    tree_sitter_xml::LANGUAGE_XML,
    tree_sitter_xml::XML_HIGHLIGHT_QUERY
);
// Upstream crates ship queries but leave HIGHLIGHTS_QUERY commented out.
config!(
    PROTO,
    tree_sitter_proto::LANGUAGE,
    include_str!("../../queries/proto.scm")
);
config!(
    DART,
    tree_sitter_dart::LANGUAGE,
    tree_sitter_dart::HIGHLIGHTS_QUERY
);
config!(R, tree_sitter_r::LANGUAGE, tree_sitter_r::HIGHLIGHTS_QUERY);
config!(
    NIX,
    tree_sitter_nix::LANGUAGE,
    tree_sitter_nix::HIGHLIGHTS_QUERY
);
config!(
    SOLIDITY,
    tree_sitter_solidity::LANGUAGE,
    tree_sitter_solidity::HIGHLIGHT_QUERY
);
config!(
    OCAML,
    tree_sitter_ocaml::LANGUAGE_OCAML,
    tree_sitter_ocaml::HIGHLIGHTS_QUERY
);
config!(
    CSHARP,
    tree_sitter_c_sharp::LANGUAGE,
    tree_sitter_c_sharp::HIGHLIGHTS_QUERY
);
config!(
    CMAKE,
    tree_sitter_cmake::LANGUAGE,
    tree_sitter_cmake::HIGHLIGHTS_QUERY
);
config!(
    GLSL,
    tree_sitter_glsl::LANGUAGE_GLSL,
    tree_sitter_glsl::HIGHLIGHTS_QUERY
);
config!(
    REGEX,
    tree_sitter_regex::LANGUAGE,
    tree_sitter_regex::HIGHLIGHTS_QUERY
);
config!(
    JSDOC,
    tree_sitter_jsdoc::LANGUAGE,
    tree_sitter_jsdoc::HIGHLIGHTS_QUERY
);
config!(
    INI,
    tree_sitter_ini::LANGUAGE,
    tree_sitter_ini::HIGHLIGHTS_QUERY
);
config!(
    SCHEME,
    tree_sitter_scheme::LANGUAGE,
    tree_sitter_scheme::HIGHLIGHTS_QUERY
);
config!(
    POWERSHELL,
    tree_sitter_powershell::LANGUAGE,
    include_str!("../../queries/powershell.scm")
);
config!(
    ELM,
    tree_sitter_elm::LANGUAGE,
    tree_sitter_elm::HIGHLIGHTS_QUERY
);
// Svelte queries use `; inherits: html` which tree-sitter-highlight does not
// expand; prepend HTML highlights explicitly.
static SVELTE: LazyLock<Option<HighlightConfiguration>> = LazyLock::new(|| {
    let svelte_q = tree_sitter_svelte_ng::HIGHLIGHTS_QUERY
        .lines()
        .filter(|line| !line.trim_start().starts_with("; inherits"))
        .collect::<Vec<_>>()
        .join("\n");
    let query = format!("{}\n{svelte_q}", tree_sitter_html::HIGHLIGHTS_QUERY);
    build(Language::new(tree_sitter_svelte_ng::LANGUAGE), &query)
});

pub(super) fn config(lang: Lang) -> Option<&'static HighlightConfiguration> {
    match lang {
        Lang::Rust => RUST.as_ref(),
        Lang::Json => JSON.as_ref(),
        Lang::Toml => TOML.as_ref(),
        Lang::Python => PYTHON.as_ref(),
        Lang::JavaScript => JAVASCRIPT.as_ref(),
        Lang::TypeScript => TYPESCRIPT.as_ref(),
        Lang::Tsx => TSX.as_ref(),
        Lang::Go => GO.as_ref(),
        Lang::Bash => BASH.as_ref(),
        Lang::Html => HTML.as_ref(),
        Lang::Css => CSS.as_ref(),
        Lang::Yaml => YAML.as_ref(),
        Lang::C => C.as_ref(),
        Lang::Cpp => CPP.as_ref(),
        Lang::Ruby => RUBY.as_ref(),
        Lang::Java => JAVA.as_ref(),
        Lang::Lua => LUA.as_ref(),
        Lang::Swift => SWIFT.as_ref(),
        Lang::Scala => SCALA.as_ref(),
        Lang::Elixir => ELIXIR.as_ref(),
        Lang::Haskell => HASKELL.as_ref(),
        Lang::Php => PHP.as_ref(),
        Lang::Zig => ZIG.as_ref(),
        Lang::Make => MAKE.as_ref(),
        Lang::Diff => DIFF.as_ref(),
        Lang::Xml => XML.as_ref(),
        Lang::Proto => PROTO.as_ref(),
        Lang::Dart => DART.as_ref(),
        Lang::R => R.as_ref(),
        Lang::Nix => NIX.as_ref(),
        Lang::Solidity => SOLIDITY.as_ref(),
        Lang::Ocaml => OCAML.as_ref(),
        Lang::CSharp => CSHARP.as_ref(),
        Lang::Cmake => CMAKE.as_ref(),
        Lang::Glsl => GLSL.as_ref(),
        Lang::Regex => REGEX.as_ref(),
        Lang::Jsdoc => JSDOC.as_ref(),
        Lang::Ini => INI.as_ref(),
        Lang::Scheme => SCHEME.as_ref(),
        Lang::Powershell => POWERSHELL.as_ref(),
        Lang::Elm => ELM.as_ref(),
        Lang::Svelte => SVELTE.as_ref(),
    }
}
