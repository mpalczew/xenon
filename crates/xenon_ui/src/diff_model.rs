//! Read-only diff model: parses `git diff` output into hunks and rows, pairs
//! removed/added lines for side-by-side layout, marks changed words, and
//! attaches syntax segments. Views draw this; they do not decide layout.

use std::path::Path;

/// Auto switches to side-by-side at or above this pane width.
pub const AUTO_SPLIT_MIN_PX: f32 = 860.;
/// Past this many diff lines the rest is cut off.
pub(crate) const MAX_LINES: usize = 6000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

/// A run of text with one syntax color key and an optional changed-word mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seg {
    pub text: String,
    pub syntax: Option<&'static str>,
    pub emphasis: bool,
}

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub segs: Vec<Seg>,
}

/// One side-by-side row: indexes into [`Hunk::lines`].
#[derive(Clone, Copy, Debug)]
pub struct SplitRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
    pub rows: Vec<SplitRow>,
}

#[derive(Clone, Debug)]
pub struct DiffSection {
    /// `("Staged", "index vs HEAD")`; none for a plain HEAD diff.
    pub title: Option<(String, String)>,
    pub left_label: String,
    pub right_label: String,
    pub hunks: Vec<Hunk>,
    pub added: u32,
    pub removed: u32,
}

#[derive(Clone, Debug, Default)]
pub struct DiffDoc {
    pub subtitle: String,
    /// Explanation above the sections (e.g. index-only files).
    pub banner: Option<String>,
    pub sections: Vec<DiffSection>,
    pub added: u32,
    pub removed: u32,
    /// Shown instead of hunks (binary file, no textual change).
    pub note: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutMode {
    #[default]
    Auto,
    Split,
    Unified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Split,
    Unified,
}

impl LayoutMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Split => "Split",
            Self::Unified => "Unified",
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::Auto => Self::Split,
            Self::Split => Self::Unified,
            Self::Unified => Self::Auto,
        }
    }

    pub(crate) fn resolve(self, width_px: f32) -> Layout {
        match self {
            Self::Split => Layout::Split,
            Self::Unified => Layout::Unified,
            Self::Auto if width_px >= AUTO_SPLIT_MIN_PX => Layout::Split,
            Self::Auto => Layout::Unified,
        }
    }
}

type RawLine = (LineKind, Option<u32>, Option<u32>, String);
type Ranges = Vec<(usize, usize)>;

/// Parse unified-diff text into hunks. Headers before the first `@@` are
/// ignored.
pub(crate) fn parse_unified(path: &str, text: &str) -> Vec<Hunk> {
    let mut raw: Vec<(String, Vec<RawLine>)> = Vec::new();
    let (mut old_no, mut new_no) = (0u32, 0u32);
    let mut total = 0;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("@@ ") {
            if total >= MAX_LINES {
                break;
            }
            let (old, new) = parse_hunk_starts(rest);
            old_no = old;
            new_no = new;
            raw.push((line.to_string(), Vec::new()));
            continue;
        }
        let Some((_, lines)) = raw.last_mut() else {
            continue;
        };
        let (kind, body) = match line.as_bytes().first() {
            Some(b'+') => (LineKind::Added, &line[1..]),
            Some(b'-') => (LineKind::Removed, &line[1..]),
            Some(b' ') => (LineKind::Context, &line[1..]),
            None => (LineKind::Context, ""),
            _ => continue,
        };
        let body = body.replace('\t', "    ");
        let entry = match kind {
            LineKind::Context => {
                let entry = (kind, Some(old_no), Some(new_no), body);
                old_no += 1;
                new_no += 1;
                entry
            }
            LineKind::Added => {
                let entry = (kind, None, Some(new_no), body);
                new_no += 1;
                entry
            }
            LineKind::Removed => {
                let entry = (kind, Some(old_no), None, body);
                old_no += 1;
                entry
            }
        };
        lines.push(entry);
        total += 1;
    }
    raw.into_iter()
        .map(|(header, lines)| build_hunk(path, header, lines))
        .collect()
}

fn parse_hunk_starts(rest: &str) -> (u32, u32) {
    let start = |prefix: char| {
        rest.split(' ')
            .find_map(|part| part.strip_prefix(prefix))
            .and_then(|range| range.split(',').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(1)
    };
    (start('-'), start('+'))
}

fn build_hunk(path: &str, header: String, raw: Vec<RawLine>) -> Hunk {
    let rows = pair_rows(&raw);
    let mut emphasis: Vec<Vec<(usize, usize)>> = vec![Vec::new(); raw.len()];
    for row in &rows {
        if let (Some(l), Some(r)) = (row.left, row.right)
            && raw[l].0 == LineKind::Removed
            && raw[r].0 == LineKind::Added
        {
            let (left, right) = changed_words(&raw[l].3, &raw[r].3);
            emphasis[l] = left;
            emphasis[r] = right;
        }
    }
    let syntax = [
        line_syntax(path, &raw, |k| k != LineKind::Added),
        line_syntax(path, &raw, |k| k != LineKind::Removed),
    ];
    let lines = raw
        .into_iter()
        .enumerate()
        .map(|(i, (kind, old_no, new_no, text))| {
            let side = usize::from(kind == LineKind::Added);
            let spans = syntax[side].get(&i).map(Vec::as_slice).unwrap_or(&[]);
            DiffLine {
                kind,
                old_no,
                new_no,
                segs: segments(&text, spans, &emphasis[i]),
            }
        })
        .collect();
    Hunk {
        header,
        lines,
        rows,
    }
}

/// Pair a run of removed lines with the added run after it, row by row.
fn pair_rows(raw: &[RawLine]) -> Vec<SplitRow> {
    let mut rows = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i].0 == LineKind::Context {
            rows.push(SplitRow {
                left: Some(i),
                right: Some(i),
            });
            i += 1;
            continue;
        }
        let removed_start = i;
        while i < raw.len() && raw[i].0 == LineKind::Removed {
            i += 1;
        }
        let added_start = i;
        while i < raw.len() && raw[i].0 == LineKind::Added {
            i += 1;
        }
        let removed = removed_start..added_start;
        let added = added_start..i;
        for n in 0..removed.len().max(added.len()) {
            rows.push(SplitRow {
                left: (n < removed.len()).then(|| removed_start + n),
                right: (n < added.len()).then(|| added_start + n),
            });
        }
    }
    rows
}

type LineSpans = std::collections::HashMap<usize, Vec<(usize, usize, &'static str)>>;

/// Syntax spans per line index, highlighting one side (old or new) of the
/// hunk as a continuous snippet.
fn line_syntax(path: &str, raw: &[RawLine], keep: impl Fn(LineKind) -> bool) -> LineSpans {
    let mut joined = String::new();
    let mut starts = Vec::new();
    for (i, (kind, _, _, text)) in raw.iter().enumerate() {
        if keep(*kind) {
            starts.push((i, joined.len(), text.len()));
            joined.push_str(text);
            joined.push('\n');
        }
    }
    let mut out = LineSpans::new();
    let spans = xenon_editor::syntax_spans_for_path(Path::new(path), &joined);
    if spans.is_empty() {
        return out;
    }
    for (i, start, len) in starts {
        let end = start + len;
        let local: Vec<_> = spans
            .iter()
            .filter(|s| s.start < end && s.end > start)
            .map(|s| (s.start.max(start) - start, s.end.min(end) - start, s.name))
            .collect();
        out.insert(i, local);
    }
    out
}

/// Cut a line into runs by syntax span and changed-word mark.
fn segments(
    text: &str,
    spans: &[(usize, usize, &'static str)],
    emphasis: &[(usize, usize)],
) -> Vec<Seg> {
    let mut cuts = vec![0, text.len()];
    for &(a, b, _) in spans {
        cuts.extend([a, b]);
    }
    for &(a, b) in emphasis {
        cuts.extend([a, b]);
    }
    cuts.retain(|&c| c <= text.len() && text.is_char_boundary(c));
    cuts.sort_unstable();
    cuts.dedup();
    let mut segs: Vec<Seg> = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let syntax = spans
            .iter()
            .rev()
            .find(|&&(s, e, _)| s <= a && e >= b)
            .map(|&(_, _, name)| name);
        let emph = emphasis.iter().any(|&(s, e)| s <= a && e >= b);
        match segs.last_mut() {
            Some(last) if last.syntax == syntax && last.emphasis == emph => {
                last.text.push_str(&text[a..b]);
            }
            _ => segs.push(Seg {
                text: text[a..b].to_string(),
                syntax,
                emphasis: emph,
            }),
        }
    }
    segs
}

/// Byte ranges that differ between two lines: what is left after trimming the
/// common word prefix and suffix. Empty when the whole line differs.
fn changed_words(old: &str, new: &str) -> (Ranges, Ranges) {
    let a = tokens(old);
    let b = tokens(new);
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x.1 == y.1).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x.1 == y.1)
        .count();
    let range = |tokens: &[(usize, &str)], text: &str| {
        let first = tokens.get(prefix).map_or(text.len(), |t| t.0);
        let end_index = tokens.len() - suffix;
        let end = tokens.get(end_index).map_or(text.len(), |t| t.0).max(first);
        (first, end)
    };
    let (la, lb) = (range(&a, old), range(&b, new));
    let whole = |r: (usize, usize), text: &str| r.0 == 0 && r.1 >= text.trim_end().len();
    if whole(la, old) && whole(lb, new) {
        return (Vec::new(), Vec::new());
    }
    let keep = |r: (usize, usize)| if r.1 > r.0 { vec![r] } else { Vec::new() };
    (keep(la), keep(lb))
}

/// Word-ish tokens with their byte offsets: runs of word characters,
/// whitespace runs, and single punctuation marks.
fn tokens(text: &str) -> Vec<(usize, &str)> {
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            1
        } else if c.is_whitespace() {
            2
        } else {
            0
        }
    };
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev: Option<(char, i32)> = None;
    for (i, c) in text.char_indices() {
        let k = class(c);
        if let Some((_, pk)) = prev
            && (k == 0 || k != pk)
        {
            out.push((start, &text[start..i]));
            start = i;
        }
        prev = Some((c, k));
    }
    if prev.is_some() {
        out.push((start, &text[start..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_pairs_changed_lines_and_marks_words() {
        let text = "diff --git a/x.txt b/x.txt\n--- a/x.txt\n+++ b/x.txt\n\
                    @@ -3,3 +3,3 @@ ctx\n keep\n-holds git flush\n+one marker flush\n tail\n";
        let hunks = parse_unified("x.txt", text);
        assert_eq!(hunks.len(), 1);
        let hunk = &hunks[0];
        assert_eq!(hunk.rows.len(), 3);
        assert_eq!(hunk.lines[1].old_no, Some(4));
        assert_eq!(hunk.lines[2].new_no, Some(4));
        let marked: Vec<_> = hunk.lines[1]
            .segs
            .iter()
            .filter(|s| s.emphasis)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(marked, ["holds git"]);
    }
}
