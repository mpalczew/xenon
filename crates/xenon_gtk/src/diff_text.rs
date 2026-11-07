//! Turns the shared diff model into text for the diff tab: one or two sides,
//! each a gutter column and a code column with the same number of lines, plus
//! the marks (line tints, changed words) the view paints with text tags. The
//! rules about rows, pairing and words come from `xenon_git`; this only
//! flattens them into lines.

use xenon_git::diff::{DiffDoc, DiffLine, Layout, LineKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tint {
    Added,
    Removed,
    /// The empty half of a split row.
    Filler,
    /// A hunk header.
    Header,
    /// Section title or column label.
    Title,
}

pub(crate) struct Mark {
    pub line: i32,
    pub tint: Tint,
}

/// A changed word inside a line (character offsets in the code column).
pub(crate) struct Word {
    pub line: i32,
    pub start: i32,
    pub end: i32,
    pub added: bool,
}

#[derive(Default)]
pub(crate) struct Side {
    pub gutter: String,
    pub code: String,
    pub marks: Vec<Mark>,
    pub words: Vec<Word>,
}

pub(crate) struct HunkAt {
    /// Line of the hunk header.
    pub line: i32,
    /// New-file line to open in the editor.
    pub target: u32,
}

#[derive(Default)]
pub(crate) struct Page {
    pub sides: Vec<Side>,
    pub hunks: Vec<HunkAt>,
    pub lines: i32,
}

struct Builder {
    page: Page,
    split: bool,
}

impl Builder {
    /// A line that is the same in every side (title, label, spacer).
    fn both(&mut self, text: &str, tint: Option<Tint>) {
        let line = self.page.lines;
        for (i, side) in self.page.sides.iter_mut().enumerate() {
            side.gutter.push_str(" \n");
            // In split mode the left side carries the text.
            if i == 0 || !self.split {
                side.code.push_str(text);
            }
            side.code.push_str(" \n");
            if let Some(tint) = tint {
                side.marks.push(Mark { line, tint });
            }
        }
        self.page.lines += 1;
    }

    fn cell(&mut self, side: usize, gutter: String, line: Option<&DiffLine>) {
        let at = self.page.lines;
        let s = &mut self.page.sides[side];
        s.gutter.push_str(&gutter);
        s.gutter.push_str(" \n");
        match line {
            Some(line) => {
                let mut offset = 0;
                for seg in &line.segs {
                    let n = seg.text.chars().count() as i32;
                    if seg.emphasis {
                        s.words.push(Word {
                            line: at,
                            start: offset,
                            end: offset + n,
                            added: line.kind == LineKind::Added,
                        });
                    }
                    offset += n;
                    s.code.push_str(&seg.text);
                }
                match line.kind {
                    LineKind::Added => s.marks.push(Mark {
                        line: at,
                        tint: Tint::Added,
                    }),
                    LineKind::Removed => s.marks.push(Mark {
                        line: at,
                        tint: Tint::Removed,
                    }),
                    LineKind::Context => {}
                }
            }
            None => s.marks.push(Mark {
                line: at,
                tint: Tint::Filler,
            }),
        }
        s.code.push_str(" \n");
    }
}

fn number(n: Option<u32>) -> String {
    n.map_or_else(|| "    ".into(), |n| format!("{n:>4}"))
}

fn sign(kind: LineKind) -> char {
    match kind {
        LineKind::Added => '+',
        LineKind::Removed => '-',
        LineKind::Context => ' ',
    }
}

pub(crate) fn build(doc: &DiffDoc, layout: Layout) -> Page {
    let split = layout == Layout::Split;
    let mut b = Builder {
        page: Page {
            sides: (0..if split { 2 } else { 1 })
                .map(|_| Side::default())
                .collect(),
            ..Page::default()
        },
        split,
    };
    for section in &doc.sections {
        if let Some((name, detail)) = &section.title {
            b.both(
                &format!(
                    "{}   {detail}   +{} -{}",
                    name.to_uppercase(),
                    section.added,
                    section.removed
                ),
                Some(Tint::Title),
            );
        }
        if split {
            let line = b.page.lines;
            for (i, label) in [&section.left_label, &section.right_label]
                .into_iter()
                .enumerate()
            {
                let side = &mut b.page.sides[i];
                side.gutter.push_str(" \n");
                side.code.push_str(label);
                side.code.push_str(" \n");
                side.marks.push(Mark {
                    line,
                    tint: Tint::Title,
                });
            }
            b.page.lines += 1;
        }
        for hunk in &section.hunks {
            b.both("", None);
            let line = b.page.lines;
            b.page.hunks.push(HunkAt {
                line,
                target: hunk.target_line(),
            });
            b.both(&hunk.header, Some(Tint::Header));
            if split {
                for row in &hunk.rows {
                    let left = row.left.map(|i| &hunk.lines[i]);
                    let right = row.right.map(|i| &hunk.lines[i]);
                    let g = |l: Option<&DiffLine>, old: bool| match l {
                        Some(l) => format!(
                            "{} {}",
                            number(if old { l.old_no } else { l.new_no }),
                            sign(l.kind)
                        ),
                        None => String::new(),
                    };
                    b.cell(0, g(left, true), left);
                    b.cell(1, g(right, false), right);
                    b.page.lines += 1;
                }
            } else {
                for l in &hunk.lines {
                    let gutter =
                        format!("{} {} {}", number(l.old_no), number(l.new_no), sign(l.kind));
                    b.cell(0, gutter, Some(l));
                    b.page.lines += 1;
                }
            }
        }
    }
    b.page
}
