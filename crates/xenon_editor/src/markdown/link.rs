//! Markdown link targets: what a link destination (or a path-like code span)
//! points at, and where links sit in the source. Resolution against the disk
//! and the workspace index happens in the app; this module only parses.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use super::markdown_options;

/// What a clicked markdown link points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// Opens in the system browser / mail client.
    Url(String),
    /// A heading in this document (GitHub-style slug, without `#`).
    Anchor(String),
    /// A file path as written (relative, absolute, or `~/`), with an optional
    /// zero-based line/column from `:12:3` or GitHub `#L12` suffixes.
    Path {
        path: String,
        line: Option<u32>,
        col: Option<u32>,
    },
}

impl LinkTarget {
    /// Classify a link destination (`[text](dest)`) or a path-like code span.
    pub fn parse(dest: &str) -> Option<Self> {
        let dest = dest.trim();
        if dest.is_empty() {
            return None;
        }
        if let Some(anchor) = dest.strip_prefix('#') {
            return Some(Self::Anchor(anchor.to_string()));
        }
        if let Some(rest) = dest.strip_prefix("file://") {
            return Some(Self::path(&percent_decode(rest)));
        }
        if is_url(dest) {
            return Some(Self::Url(dest.to_string()));
        }
        Some(Self::path(&percent_decode(dest)))
    }

    fn path(raw: &str) -> Self {
        let raw = raw.split('?').next().unwrap_or(raw);
        let (raw, fragment) = match raw.split_once('#') {
            Some((head, fragment)) => (head, Some(fragment)),
            None => (raw, None),
        };
        let (path, mut line, mut col) = split_line_suffix(raw);
        if let Some((l, c)) = fragment.and_then(github_line) {
            line = Some(l);
            col = c;
        }
        Self::Path {
            path: path.to_string(),
            line,
            col,
        }
    }
}

/// `https://…`, `mailto:…`: a scheme followed by `//`, or a known opaque scheme.
/// `file.rs:12` must not read as scheme `file.rs`.
fn is_url(dest: &str) -> bool {
    let Some((scheme, rest)) = dest.split_once(':') else {
        return false;
    };
    let valid = scheme.len() > 1
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid && (rest.starts_with("//") || matches!(scheme, "mailto" | "tel" | "sms"))
}

/// Strip `:line` / `:line:col` (one-based) into zero-based numbers.
fn split_line_suffix(raw: &str) -> (&str, Option<u32>, Option<u32>) {
    let mut numbers = Vec::new();
    let mut head = raw;
    while numbers.len() < 2 {
        match head.rsplit_once(':') {
            Some((h, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => {
                numbers.push(tail.parse::<u32>().unwrap_or(1));
                head = h;
            }
            _ => break,
        }
    }
    numbers.reverse();
    let zero = |n: Option<&u32>| n.map(|n| n.saturating_sub(1));
    (head, zero(numbers.first()), zero(numbers.get(1)))
}

/// GitHub `#L12`, `#L12-L20`, `#L12C3`: zero-based line and optional column.
fn github_line(fragment: &str) -> Option<(u32, Option<u32>)> {
    let rest = fragment.strip_prefix('L')?;
    let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
    let n = digits(rest);
    let line: u32 = rest[..n].parse().ok()?;
    let col = rest[n..]
        .strip_prefix('C')
        .and_then(|c| c[..digits(c)].parse::<u32>().ok())
        .map(|c| c.saturating_sub(1));
    Some((line.saturating_sub(1), col))
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((hi * 16 + lo) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

/// An inline code span that names a file: `docs/release.md`, `src/main.rs:40`,
/// `~/bin/xenon`. Needs a separator or a file extension and no spaces, so
/// `cargo`, `--profile dogfood` and `Vec<T>` stay plain code.
pub fn code_is_path(code: &str) -> bool {
    let code = code.trim();
    if code.is_empty() || code.len() > 240 || code.contains(char::is_whitespace) {
        return false;
    }
    if code.contains(|c: char| "<>{}()[]|*?\"'`=,;$!@".contains(c)) || code.starts_with('-') {
        return false;
    }
    if code.contains("::") || code.ends_with('/') || is_url(code) {
        return false;
    }
    let (path, _, _) = split_line_suffix(code);
    let name = path.rsplit('/').next().unwrap_or(path);
    let has_ext = name.rsplit_once('.').is_some_and(|(_, ext)| {
        !ext.is_empty()
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
            && !ext.chars().all(|c| c.is_ascii_digit())
    });
    let has_dir = path.contains('/') && path.chars().any(|c| c.is_ascii_alphabetic());
    has_dir || has_ext
}

/// GitHub heading slug: lowercase, spaces to `-`, punctuation dropped.
pub fn slug(title: &str) -> String {
    title
        .trim()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

/// The link destination under byte `offset` of `source`: a `[text](dest)` link
/// (inline, reference, or autolink) or a path-like code span.
pub fn link_at(source: &str, offset: usize) -> Option<String> {
    let mut open: Vec<(Range<usize>, String)> = Vec::new();
    for (event, range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        if range.start > offset {
            break;
        }
        let covers = range.start <= offset && offset < range.end;
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => open.push((range, dest_url.to_string())),
            Event::End(TagEnd::Link) => {
                if let Some((range, dest)) = open.pop()
                    && range.contains(&offset)
                {
                    return Some(dest);
                }
            }
            Event::Code(code) if covers && open.is_empty() && code_is_path(&code) => {
                return Some(code.to_string());
            }
            _ => {}
        }
    }
    open.into_iter()
        .rev()
        .find(|(range, _)| range.contains(&offset))
        .map(|(_, dest)| dest)
}

/// Byte offset of the heading whose slug is `anchor`.
pub fn heading_offset(source: &str, anchor: &str) -> Option<usize> {
    let want = slug(&percent_decode(anchor));
    let mut heading: Option<(usize, String)> = None;
    for (event, range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some((range.start, String::new())),
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, title)) = &mut heading {
                    title.push_str(&text);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((start, title)) = heading.take()
                    && slug(&title) == want
                {
                    return Some(start);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(p: &str, line: Option<u32>, col: Option<u32>) -> Option<LinkTarget> {
        Some(LinkTarget::Path {
            path: p.into(),
            line,
            col,
        })
    }

    #[test]
    fn parses_link_destinations() {
        assert_eq!(
            LinkTarget::parse("https://x.dev/a"),
            Some(LinkTarget::Url("https://x.dev/a".into()))
        );
        assert_eq!(
            LinkTarget::parse("mailto:a@b.c"),
            Some(LinkTarget::Url("mailto:a@b.c".into()))
        );
        assert_eq!(
            LinkTarget::parse("#build-and-check"),
            Some(LinkTarget::Anchor("build-and-check".into()))
        );
        assert_eq!(
            LinkTarget::parse("src/main.rs:12:3"),
            path("src/main.rs", Some(11), Some(2))
        );
        assert_eq!(
            LinkTarget::parse("file.rs:12"),
            path("file.rs", Some(11), None)
        );
        assert_eq!(
            LinkTarget::parse("a/b.rs#L40-L52"),
            path("a/b.rs", Some(39), None)
        );
        assert_eq!(
            LinkTarget::parse("a/b.rs#L4C7"),
            path("a/b.rs", Some(3), Some(6))
        );
        assert_eq!(
            LinkTarget::parse("docs/x.md#usage"),
            path("docs/x.md", None, None)
        );
        assert_eq!(
            LinkTarget::parse("My%20Notes.md"),
            path("My Notes.md", None, None)
        );
        assert_eq!(
            LinkTarget::parse("file:///tmp/a.md"),
            path("/tmp/a.md", None, None)
        );
    }

    #[test]
    fn code_spans_that_name_files() {
        for yes in [
            "docs/project-state.md",
            "scripts/health",
            ".vscode/tasks.json",
            "~/bin/xenon",
            "ATTRIBUTION.md",
            "src/main.rs:40",
        ] {
            assert!(code_is_path(yes), "{yes}");
        }
        for no in [
            "cargo",
            "--profile dogfood",
            "Vec<T>",
            "foo::bar",
            "KillMode=process",
            "1.5",
            "https://x.dev",
            "a/",
        ] {
            assert!(!code_is_path(no), "{no}");
        }
    }

    #[test]
    fn finds_link_under_offset() {
        let src = "See [the docs](docs/a.md) and `src/b.rs` or `cargo`.\n";
        let at = |needle: &str| link_at(src, src.find(needle).unwrap());
        assert_eq!(at("the docs").as_deref(), Some("docs/a.md"));
        assert_eq!(at("docs/a.md").as_deref(), Some("docs/a.md"));
        assert_eq!(at("src/b.rs").as_deref(), Some("src/b.rs"));
        assert_eq!(at("cargo"), None);
        assert_eq!(at("See"), None);
    }

    #[test]
    fn heading_slugs_resolve() {
        let src = "# Title\n\n## Build and check\n\ntext\n";
        assert_eq!(heading_offset(src, "build-and-check"), src.find("## Build"));
        assert_eq!(heading_offset(src, "missing"), None);
    }
}
