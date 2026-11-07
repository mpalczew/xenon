//! The diff tab's model: the two texts of one file (HEAD against the working
//! tree, or HEAD against the index when the tree matches HEAD) and the lines
//! of each side, computed with `similar`. The view only draws these lines.

use std::path::Path;
use std::process::Command;

use similar::{ChangeTag, DiffOp, InlineChange, TextDiff};
use xenon_git::status::GitFile;

/// Auto switches to side-by-side at or above this pane width.
const AUTO_SPLIT_MIN_PX: f32 = 860.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LayoutMode {
    #[default]
    Auto,
    Split,
    Unified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layout {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Context,
    Added,
    Removed,
    /// The empty half of a split row.
    Filler,
    /// A hunk's `@@` line.
    Header,
}

#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub text: String,
    pub kind: Kind,
    /// 1-based line numbers in the old and new text; 0 for none.
    pub old: u32,
    pub new: u32,
    /// Changed words, as character ranges in `text`.
    pub words: Vec<(i32, i32)>,
}

impl Line {
    fn filler() -> Self {
        Self::plain("", Kind::Filler, 0, 0)
    }

    fn plain(text: &str, kind: Kind, old: u32, new: u32) -> Self {
        let text = text.trim_end_matches(['\r', '\n']).to_string();
        Self {
            text,
            kind,
            old,
            new,
            words: Vec::new(),
        }
    }

    fn of_change(change: &InlineChange<str>) -> Self {
        let number = |i: Option<usize>| i.map_or(0, |i| i as u32 + 1);
        let kind = if change.tag() == ChangeTag::Delete {
            Kind::Removed
        } else {
            Kind::Added
        };
        let mut line = Self::plain(
            "",
            kind,
            number(change.old_index()),
            number(change.new_index()),
        );
        let mut at = 0;
        for (changed, part) in change.iter_strings_lossy() {
            let part = part.trim_end_matches(['\r', '\n']);
            let len = part.chars().count() as i32;
            if changed && len > 0 {
                line.words.push((at, at + len));
            }
            line.text.push_str(part);
            at += len;
        }
        line
    }
}

/// One change block: the line to show it at and the new-file line to open.
pub(crate) struct Hunk {
    pub line: i32,
    pub target: u32,
}

#[derive(Default)]
pub(crate) struct Page {
    /// One side when unified, two when split; equal length in split.
    pub sides: Vec<Vec<Line>>,
    pub hunks: Vec<Hunk>,
}

#[derive(Default)]
pub(crate) struct Doc {
    pub subtitle: String,
    pub banner: Option<String>,
    pub note: Option<String>,
    pub added: usize,
    pub removed: usize,
    old: String,
    new: String,
}

impl Doc {
    fn of(subtitle: &str, old: &str, new: &str) -> Self {
        let diff = TextDiff::from_lines(old, new);
        let (mut added, mut removed) = (0, 0);
        for op in diff
            .ops()
            .iter()
            .filter(|o| !matches!(o, DiffOp::Equal { .. }))
        {
            removed += op.old_range().len();
            added += op.new_range().len();
        }
        Self {
            subtitle: subtitle.into(),
            added,
            removed,
            old: old.into(),
            new: new.into(),
            ..Self::default()
        }
    }

    fn note(subtitle: &str, note: String) -> Self {
        Self {
            subtitle: subtitle.into(),
            note: Some(note),
            ..Self::default()
        }
    }

    /// The lines of each side for a layout.
    pub(crate) fn build(&self, layout: Layout) -> Page {
        let split = layout == Layout::Split;
        let diff = TextDiff::from_lines(self.old.as_str(), self.new.as_str());
        let mut page = Page {
            sides: vec![Vec::new(); if split { 2 } else { 1 }],
            hunks: Vec::new(),
        };
        for group in diff.grouped_ops(3) {
            let range = |old: bool| {
                let r = |op: &DiffOp| if old { op.old_range() } else { op.new_range() };
                let (start, end) = (r(&group[0]).start, r(&group[group.len() - 1]).end);
                let first = if end == start { start } else { start + 1 };
                match end - start {
                    1 => first.to_string(),
                    len => format!("{first},{len}"),
                }
            };
            let header = format!("@@ -{} +{} @@", range(true), range(false));
            let at = page.sides[0].len() as i32;
            for side in &mut page.sides {
                side.push(Line::plain(&header, Kind::Header, 0, 0));
            }
            let first_change = group.iter().find(|o| !matches!(o, DiffOp::Equal { .. }));
            page.hunks.push(Hunk {
                line: at,
                target: first_change.map_or(0, |o| o.new_range().start as u32 + 1),
            });
            for op in &group {
                if let DiffOp::Equal {
                    old_index,
                    new_index,
                    len,
                } = *op
                {
                    for i in 0..len {
                        let line = Line::plain(
                            diff.old_slice(old_index + i).unwrap_or_default(),
                            Kind::Context,
                            (old_index + i + 1) as u32,
                            (new_index + i + 1) as u32,
                        );
                        page.sides.iter_mut().for_each(|s| s.push(line.clone()));
                    }
                    continue;
                }
                let (removed, added): (Vec<_>, Vec<_>) = diff
                    .iter_inline_changes(op)
                    .map(|c| Line::of_change(&c))
                    .partition(|l| l.kind == Kind::Removed);
                if split {
                    for i in 0..removed.len().max(added.len()) {
                        let pick = |v: &[Line]| v.get(i).cloned().unwrap_or_else(Line::filler);
                        page.sides[0].push(pick(&removed));
                        page.sides[1].push(pick(&added));
                    }
                } else {
                    page.sides[0].extend(removed);
                    page.sides[0].extend(added);
                }
            }
        }
        page
    }
}

/// A blob from git (`HEAD:path`, `:path`); none when git has no such blob.
fn show(root: &Path, spec: &str) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .args(["show", spec])
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "1")
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// The diff for one status entry. Blocking: call off the UI thread.
pub(crate) fn load(root: &Path, file: &GitFile) -> Doc {
    let path = file.path.as_str();
    let work = std::fs::read(root.join(path));
    if file.is_untracked() {
        return match work {
            Ok(bytes) if bytes.contains(&0) => Doc::note(
                "untracked \u{b7} new file",
                "Binary file, not shown.".into(),
            ),
            Ok(bytes) => Doc::of(
                "untracked \u{b7} new file",
                "",
                &String::from_utf8_lossy(&bytes),
            ),
            Err(error) => Doc::note("", format!("Can\u{2019}t read {path}: {error}")),
        };
    }
    let head = show(
        root,
        &format!("HEAD:{}", file.from.as_deref().unwrap_or(path)),
    );
    let index = show(root, &format!(":{path}"));
    let (head, index, work) = (
        head.unwrap_or_default(),
        index.unwrap_or_default(),
        work.unwrap_or_default(),
    );
    if [&head, &index, &work].iter().any(|b| b.contains(&0)) {
        return Doc::note(
            "HEAD \u{2192} working tree",
            "Binary file, not shown.".into(),
        );
    }
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    if head != work {
        return Doc::of("HEAD \u{2192} working tree", &text(&head), &text(&work));
    }
    let subtitle = "HEAD \u{21c4} working tree \u{b7} no difference";
    if index == head {
        return Doc::note(subtitle, "No textual changes.".into());
    }
    Doc {
        banner: Some(
            "Working tree matches HEAD. Staged and unstaged changes cancel out, so the \
             net diff against HEAD is empty."
                .into(),
        ),
        ..Doc::of(subtitle, &text(&head), &text(&index))
    }
}
