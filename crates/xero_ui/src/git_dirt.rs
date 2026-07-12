//! Per-workspace git dirtiness: line insertions/deletions vs HEAD,
//! including untracked (non-ignored) files as pure insertions.

use std::path::Path;
use std::process::Command;

use gpui::{IntoElement, ParentElement, Styled, div};

/// Line-level dirty stats for a workspace root that is a git work tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GitDirt {
    pub insertions: u64,
    pub deletions: u64,
}

impl GitDirt {
    pub fn is_clean(self) -> bool {
        self.insertions == 0 && self.deletions == 0
    }

    /// Compact label parts for the sidebar (`+12`, `-3`). Empty when clean.
    pub fn labels(self) -> Option<(String, String)> {
        if self.is_clean() {
            return None;
        }
        let plus = if self.insertions > 0 {
            format!("+{}", self.insertions)
        } else {
            String::new()
        };
        let minus = if self.deletions > 0 {
            format!("-{}", self.deletions)
        } else {
            String::new()
        };
        Some((plus, minus))
    }
}

/// Sum `git diff --numstat` lines. Binary rows (`-  -  path`) are skipped.
pub fn parse_numstat(output: &str) -> GitDirt {
    let mut dirt = GitDirt::default();
    for line in output.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(added), Some(removed)) = (parts.next(), parts.next()) else {
            continue;
        };
        if let Ok(n) = added.parse::<u64>() {
            dirt.insertions += n;
        }
        if let Ok(n) = removed.parse::<u64>() {
            dirt.deletions += n;
        }
    }
    dirt
}

/// Count lines in file bytes the way git numstat does for new text files.
/// Binary (NUL in content) yields `None`.
pub fn count_text_lines(bytes: &[u8]) -> Option<u64> {
    if bytes.contains(&0) {
        return None;
    }
    if bytes.is_empty() {
        return Some(0);
    }
    let text = std::str::from_utf8(bytes).ok()?;
    Some(text.lines().count() as u64)
}

/// Tracked staged+unstaged vs HEAD, plus untracked non-ignored file lines as +.
/// `None` if `root` is not a git work tree (or git is missing).
pub fn measure(root: &Path) -> Option<GitDirt> {
    if !is_work_tree(root) {
        return None;
    }
    let mut dirt = tracked_dirt(root);
    dirt.insertions += untracked_insertions(root);
    Some(dirt)
}

fn is_work_tree(root: &Path) -> bool {
    git(root, &["rev-parse", "--is-inside-work-tree"]).is_some_and(|out| out.trim() == "true")
}

fn tracked_dirt(root: &Path) -> GitDirt {
    git(root, &["diff", "--numstat", "HEAD", "--", "."])
        .map(|out| parse_numstat(&out))
        .unwrap_or_default()
}

fn untracked_insertions(root: &Path) -> u64 {
    let listing = match git(root, &["ls-files", "--others", "--exclude-standard", "-z"]) {
        Some(s) => s,
        None => return 0,
    };
    listing
        .split('\0')
        .filter(|p| !p.is_empty())
        .filter_map(|rel| count_text_lines(&std::fs::read(root.join(rel)).ok()?))
        .sum()
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "1")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Compact `+N -M` chip for the workspace sidebar header.
/// Two fixed right-aligned columns so counts line up across workspace rows.
pub fn badge(
    plus: String,
    minus: String,
    added: gpui::Hsla,
    deleted: gpui::Hsla,
) -> impl IntoElement {
    use gpui::px;
    div()
        .flex()
        .items_center()
        .gap(px(3.))
        .flex_none()
        .text_xs()
        .font_weight(gpui::FontWeight::NORMAL)
        .child(stat_col(plus, added))
        .child(stat_col(minus, deleted))
}

fn stat_col(label: String, color: gpui::Hsla) -> impl IntoElement {
    use gpui::px;
    div()
        .w(px(36.))
        .flex_none()
        .flex()
        .justify_end()
        .text_color(color)
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::{fs, path::PathBuf};

    #[test]
    fn parse_sums_rows_and_skips_binary() {
        let dirt = parse_numstat(
            "3\t1\ta.rs\n\
             0\t5\tb.rs\n\
             -\t-\tbin.bin\n\
             10\t0\tc.rs\n",
        );
        assert_eq!(
            dirt,
            GitDirt {
                insertions: 13,
                deletions: 6,
            }
        );
    }

    #[test]
    fn labels_omit_zero_side() {
        assert_eq!(GitDirt::default().labels(), None);
        assert_eq!(
            GitDirt {
                insertions: 4,
                deletions: 0
            }
            .labels(),
            Some(("+4".into(), String::new()))
        );
        assert_eq!(
            GitDirt {
                insertions: 0,
                deletions: 9
            }
            .labels(),
            Some((String::new(), "-9".into()))
        );
        assert_eq!(
            GitDirt {
                insertions: 1,
                deletions: 2
            }
            .labels(),
            Some(("+1".into(), "-2".into()))
        );
    }

    #[test]
    fn text_line_count_matches_git_style() {
        assert_eq!(count_text_lines(b""), Some(0));
        assert_eq!(count_text_lines(b"a"), Some(1));
        assert_eq!(count_text_lines(b"a\n"), Some(1));
        assert_eq!(count_text_lines(b"a\nb"), Some(2));
        assert_eq!(count_text_lines(b"a\nb\n"), Some(2));
        assert_eq!(count_text_lines(b"a\0b"), None);
    }

    #[test]
    fn measure_counts_untracked_lines() {
        let root = temp_repo();
        fs::write(root.join("tracked.txt"), "1\n2\n3\n").unwrap();
        run(&root, &["add", "tracked.txt"]);
        run(&root, &["commit", "-m", "init"]);
        fs::write(root.join("tracked.txt"), "1\n2\n3\n4\n").unwrap(); // +1 tracked
        fs::write(root.join("new.txt"), "a\nb\nc\n").unwrap(); // +3 untracked
        fs::write(root.join("bin.dat"), [0u8, 1, 2]).unwrap(); // binary, skip
        let dirt = measure(&root).expect("work tree");
        assert_eq!(
            dirt,
            GitDirt {
                insertions: 4,
                deletions: 0
            }
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn temp_repo() -> PathBuf {
        let root = std::env::temp_dir().join(format!("xero-git-dirt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        run(&root, &["init"]);
        run(&root, &["config", "user.email", "test@example.com"]);
        run(&root, &["config", "user.name", "test"]);
        root
    }

    fn run(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?} failed");
    }
}
