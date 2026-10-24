//! Per-workspace git state from `git status --porcelain=v1 -z`: the dirt
//! level for the sidebar marker, plus the changed-file list with per-file
//! +/- counts for the Git section.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// How far a workspace has drifted from HEAD.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DirtState {
    #[default]
    Clean,
    /// The index differs from HEAD but the working tree matches HEAD
    /// (`git diff HEAD` is empty yet `git status` lists files).
    IndexOnly,
    /// The working tree differs from HEAD, or has untracked files.
    Dirty,
}

/// What a status letter (or a pair of them) means for color and badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflict,
}

impl ChangeKind {
    fn of(letter: char) -> Option<Self> {
        match letter {
            'M' | 'T' => Some(Self::Modified),
            'A' => Some(Self::Added),
            'D' => Some(Self::Deleted),
            'R' | 'C' => Some(Self::Renamed),
            '?' => Some(Self::Untracked),
            'U' => Some(Self::Conflict),
            _ => None,
        }
    }
}

/// One `git status` entry with its change size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitFile {
    /// Index column (`X`); a space when the index is untouched.
    pub index: char,
    /// Working-tree column (`Y`); a space when the tree matches the index.
    pub worktree: char,
    pub path: String,
    /// Rename/copy source.
    pub from: Option<String>,
    pub added: u32,
    pub removed: u32,
}

impl GitFile {
    pub(crate) fn is_untracked(&self) -> bool {
        self.index == '?'
    }

    pub(crate) fn is_conflict(&self) -> bool {
        matches!(
            (self.index, self.worktree),
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D')
        )
    }

    /// Overall kind, used to color the file name.
    pub(crate) fn kind(&self) -> ChangeKind {
        if self.is_conflict() {
            return ChangeKind::Conflict;
        }
        if self.is_untracked() {
            return ChangeKind::Untracked;
        }
        let (index, tree) = (ChangeKind::of(self.index), ChangeKind::of(self.worktree));
        [ChangeKind::Added, ChangeKind::Deleted, ChangeKind::Renamed]
            .into_iter()
            .find(|k| index == Some(*k) || tree == Some(*k))
            .or(index)
            .or(tree)
            .unwrap_or(ChangeKind::Modified)
    }

    /// Badge cells in display order: index then working tree (only the set
    /// ones; an untracked file gets a single `?`).
    pub(crate) fn badges(&self) -> Vec<(char, ChangeKind)> {
        if self.is_untracked() {
            return vec![('?', ChangeKind::Untracked)];
        }
        [self.index, self.worktree]
            .into_iter()
            .filter_map(|c| ChangeKind::of(c).map(|k| (c, k)))
            .collect()
    }

    pub(crate) fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The deepest directories that fit the sidebar's ~10-character room:
    /// the last two segments (`src/app`) when they fit, else the last segment
    /// alone (`sidebar`), else its tail (`…ar_footer`). The row truncates at
    /// the end, so the distinguishing leaf must never be what gets cut.
    pub(crate) fn dir_tail(&self) -> String {
        const BUDGET: usize = 10;
        let parts: Vec<&str> = self.dir().split('/').filter(|p| !p.is_empty()).collect();
        let tail = &parts[parts.len().saturating_sub(2)..];
        let joined = tail.join("/");
        if joined.chars().count() <= BUDGET {
            return joined;
        }
        let last = tail.last().copied().unwrap_or_default();
        let n = last.chars().count();
        if n <= BUDGET {
            return last.to_string();
        }
        let keep: String = last.chars().skip(n - (BUDGET - 1)).collect();
        format!("…{keep}")
    }

    /// Directory part of the path, without a trailing slash (empty at root).
    pub(crate) fn dir(&self) -> &str {
        self.path
            .rsplit_once('/')
            .map(|(dir, _)| dir)
            .unwrap_or_default()
    }
}

/// One cell of a five-cell change bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BarCell {
    Added,
    Removed,
    Neutral,
}

/// Five cells sized by the share of added and removed lines (at least one
/// cell for each side that changed).
pub(crate) fn change_bar(added: u32, removed: u32) -> [BarCell; 5] {
    let total = added + removed;
    let mut cells = [BarCell::Neutral; 5];
    if total == 0 {
        return cells;
    }
    let most = if added > 0 && removed > 0 { 4 } else { 5 };
    let share = |n: u32| {
        if n == 0 {
            0
        } else {
            ((5 * n + total / 2) / total).clamp(1, most) as usize
        }
    };
    let (mut green, mut red) = (share(added), share(removed));
    while green + red > 5 {
        if green >= red {
            green -= 1;
        } else {
            red -= 1;
        }
    }
    for (i, cell) in cells.iter_mut().enumerate() {
        if i < green {
            *cell = BarCell::Added;
        } else if i < green + red {
            *cell = BarCell::Removed;
        }
    }
    cells
}

/// Git state of one workspace root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub branch: String,
    pub state: DirtState,
    pub files: Vec<GitFile>,
}

/// Status plus change counts. `None` if `root` is not a git work tree (or git
/// is missing).
pub(crate) fn measure(root: &Path) -> Option<GitStatus> {
    if !is_work_tree(root) {
        return None;
    }
    let raw = git_bytes(
        root,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--branch",
            "-uall",
        ],
    )?;
    let (branch, mut files) = parse_status_z(&raw);
    if files.is_empty() {
        return Some(GitStatus {
            branch,
            state: DirtState::Clean,
            files,
        });
    }
    let head_matches = Command::new("git")
        .args(["diff", "HEAD", "--quiet", "--", "."])
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "1")
        .status()
        .is_ok_and(|status| status.code() == Some(0));
    let state = if head_matches && !files.iter().any(GitFile::is_untracked) {
        DirtState::IndexOnly
    } else {
        DirtState::Dirty
    };
    fill_counts(root, &mut files);
    Some(GitStatus {
        branch,
        state,
        files,
    })
}

fn fill_counts(root: &Path, files: &mut [GitFile]) {
    let head = numstat(root, &["diff", "HEAD", "--numstat", "-z", "-M", "--", "."]);
    let mut staged: Option<HashMap<String, (u32, u32)>> = None;
    let mut unstaged: Option<HashMap<String, (u32, u32)>> = None;
    for file in files {
        if file.is_untracked() {
            let counts = std::fs::metadata(root.join(&file.path))
                .ok()
                .filter(|meta| meta.len() <= 2_000_000)
                .and_then(|_| std::fs::read(root.join(&file.path)).ok())
                .and_then(|bytes| count_text_lines(&bytes));
            file.added = counts.unwrap_or(0) as u32;
        } else if let Some(&(added, removed)) = head.get(&file.path) {
            file.added = added;
            file.removed = removed;
        } else {
            let staged = staged.get_or_insert_with(|| {
                numstat(
                    root,
                    &["diff", "--cached", "--numstat", "-z", "-M", "--", "."],
                )
            });
            let unstaged = unstaged
                .get_or_insert_with(|| numstat(root, &["diff", "--numstat", "-z", "--", "."]));
            for map in [&*staged, &*unstaged] {
                if let Some(&(added, removed)) = map.get(&file.path) {
                    file.added += added;
                    file.removed += removed;
                }
            }
        }
    }
}

fn is_work_tree(root: &Path) -> bool {
    git_bytes(root, &["rev-parse", "--is-inside-work-tree"])
        .is_some_and(|out| String::from_utf8_lossy(&out).trim() == "true")
}

fn numstat(root: &Path, args: &[&str]) -> HashMap<String, (u32, u32)> {
    git_bytes(root, args)
        .map(|out| parse_numstat_z(&out))
        .unwrap_or_default()
}

fn git_bytes(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "1")
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// Parse `git status --porcelain=v1 -z --branch`: the `## branch` record,
/// then `XY path` records (renames and copies are followed by their source).
pub(crate) fn parse_status_z(raw: &[u8]) -> (String, Vec<GitFile>) {
    let text = String::from_utf8_lossy(raw);
    let mut records = text.split('\0');
    let mut branch = String::new();
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        if let Some(head) = record.strip_prefix("## ") {
            branch = parse_branch(head);
            continue;
        }
        let mut chars = record.chars();
        let (Some(index), Some(worktree)) = (chars.next(), chars.next()) else {
            continue;
        };
        let Some(path) = record.get(3..) else {
            continue;
        };
        if index == '!' {
            continue;
        }
        let from = matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C');
        files.push(GitFile {
            index,
            worktree,
            path: path.to_string(),
            from: from.then(|| records.next().map(str::to_string)).flatten(),
            added: 0,
            removed: 0,
        });
    }
    (branch, files)
}

fn parse_branch(head: &str) -> String {
    if let Some(name) = head.strip_prefix("No commits yet on ") {
        return name.to_string();
    }
    if head.starts_with("HEAD (no branch)") {
        return "detached".to_string();
    }
    head.split("...")
        .next()
        .unwrap_or(head)
        .split(' ')
        .next()
        .unwrap_or(head)
        .to_string()
}

/// Parse `git diff --numstat -z`. Binary rows (`-  -`) count as zero.
/// Renames arrive as `a\tr\t\0old\0new\0` and are keyed by the new path.
pub(crate) fn parse_numstat_z(raw: &[u8]) -> HashMap<String, (u32, u32)> {
    let text = String::from_utf8_lossy(raw);
    let mut records = text.split('\0');
    let mut out = HashMap::new();
    while let Some(record) = records.next() {
        let mut parts = record.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let counts = (added.parse().unwrap_or(0), removed.parse().unwrap_or(0));
        let path = if path.is_empty() {
            let _old = records.next();
            records.next().unwrap_or_default()
        } else {
            path
        };
        out.insert(path.to_string(), counts);
    }
    out
}

/// Count lines in file bytes the way git numstat does for new text files.
/// Binary (NUL in content) yields `None`.
pub(crate) fn count_text_lines(bytes: &[u8]) -> Option<u64> {
    if bytes.contains(&0) {
        return None;
    }
    if bytes.is_empty() {
        return Some(0);
    }
    let text = std::str::from_utf8(bytes).ok()?;
    Some(text.lines().count() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_z_parses_branch_pairs_and_renames() {
        let raw =
            b"## main...origin/main [ahead 1]\0MM a/b.rs\0A  new.rs\0R  to.md\0from.md\0?? t.txt\0";
        let (branch, files) = parse_status_z(raw);
        assert_eq!(branch, "main");
        assert_eq!(files.len(), 4);
        assert_eq!((files[0].index, files[0].worktree), ('M', 'M'));
        assert_eq!(files[0].dir(), "a");
        assert_eq!(files[2].from.as_deref(), Some("from.md"));
        assert!(files[3].is_untracked());
        let counts = parse_numstat_z(b"3\t1\ta.rs\0-\t-\tbin\x002\t0\t\0old\0new\0");
        assert_eq!(counts["a.rs"], (3, 1));
        assert_eq!(counts["bin"], (0, 0));
        assert_eq!(counts["new"], (2, 0));
    }

    #[test]
    fn text_line_count_matches_git_style() {
        assert_eq!(count_text_lines(b""), Some(0));
        assert_eq!(count_text_lines(b"a"), Some(1));
        assert_eq!(count_text_lines(b"a\n"), Some(1));
        assert_eq!(count_text_lines(b"a\nb"), Some(2));
        assert_eq!(count_text_lines(b"a\0b"), None);
    }
}
