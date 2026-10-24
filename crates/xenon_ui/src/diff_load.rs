//! Loads the diff for one status entry from git and the working tree.

use std::path::Path;
use std::process::Command;

use crate::diff_model::{DiffDoc, DiffSection, LineKind, MAX_LINES, parse_unified};
use crate::git_dirt::GitFile;

/// Diff for one status entry, read from git and the working tree. Blocking:
/// call off the UI thread.
pub(crate) fn load(root: &Path, file: &GitFile) -> DiffDoc {
    let path = file.path.as_str();
    if file.is_untracked() {
        return match std::fs::read(root.join(path)) {
            Ok(bytes) => all_added(path, &bytes),
            Err(error) => note_doc(format!("Can’t read {path}: {error}")),
        };
    }
    let mut paths = vec![path];
    if let Some(from) = &file.from {
        paths.insert(0, from);
    }
    let against_head = git_diff(root, &["HEAD"], &paths);
    let (staged, unstaged) = if against_head.trim().is_empty() {
        (
            git_diff(root, &["--cached"], &paths),
            git_diff(root, &[], &paths),
        )
    } else {
        Default::default()
    };
    compose(path, &against_head, &staged, &unstaged)
}

/// Build the document from git's three views of one file: HEAD against the
/// working tree, and (when that is empty) the staged and unstaged halves.
pub(crate) fn compose(path: &str, against_head: &str, staged: &str, unstaged: &str) -> DiffDoc {
    if !against_head.trim().is_empty() {
        let section = section(None, "HEAD", "Working tree", path, against_head);
        return doc(
            "HEAD → working tree",
            None,
            vec![section],
            binary_note(against_head),
        );
    }
    if staged.trim().is_empty() && unstaged.trim().is_empty() {
        return doc(
            "HEAD ⇄ working tree · no difference",
            None,
            Vec::new(),
            Some("No textual changes.".into()),
        );
    }
    let sections = vec![
        section(
            Some(("Staged", "index vs HEAD")),
            "HEAD",
            "Index (staged)",
            path,
            staged,
        ),
        section(
            Some(("Unstaged", "working tree vs index")),
            "Index (staged)",
            "Working tree (= HEAD)",
            path,
            unstaged,
        ),
    ];
    doc(
        "HEAD ⇄ working tree · no difference",
        Some(
            "Working tree matches HEAD. Staged and unstaged changes cancel out, so the \
             net diff against HEAD is empty."
                .into(),
        ),
        sections,
        None,
    )
}

fn git_diff(root: &Path, mode: &[&str], paths: &[&str]) -> String {
    let output = Command::new("git")
        .args(["diff", "--no-color", "--no-ext-diff", "-U3", "-M"])
        .args(mode)
        .arg("--")
        .args(paths)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "1")
        .output();
    output
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

fn binary_note(text: &str) -> Option<String> {
    (text.contains("Binary files ") && !text.contains("\n@@"))
        .then(|| "Binary file, not shown.".to_string())
}

fn note_doc(note: String) -> DiffDoc {
    doc("", None, Vec::new(), Some(note))
}

fn doc(
    subtitle: &str,
    banner: Option<String>,
    sections: Vec<DiffSection>,
    note: Option<String>,
) -> DiffDoc {
    let truncated = sections
        .iter()
        .map(|s| s.hunks.iter().map(|h| h.lines.len()).sum::<usize>())
        .sum::<usize>()
        >= MAX_LINES;
    DiffDoc {
        subtitle: subtitle.to_string(),
        banner,
        added: sections.iter().map(|s| s.added).sum(),
        removed: sections.iter().map(|s| s.removed).sum(),
        sections,
        note,
        truncated,
    }
}

fn section(
    title: Option<(&str, &str)>,
    left: &str,
    right: &str,
    path: &str,
    unified: &str,
) -> DiffSection {
    let hunks = parse_unified(path, unified);
    let count = |kind| {
        hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| l.kind == kind)
            .count() as u32
    };
    DiffSection {
        title: title.map(|(a, b)| (a.to_string(), b.to_string())),
        left_label: left.to_string(),
        right_label: right.to_string(),
        added: count(LineKind::Added),
        removed: count(LineKind::Removed),
        hunks,
    }
}

/// Untracked or added file: every line is an addition.
pub(crate) fn all_added(path: &str, bytes: &[u8]) -> DiffDoc {
    if bytes.contains(&0) {
        return doc(
            "untracked · new file",
            None,
            Vec::new(),
            Some("Binary file, not shown.".into()),
        );
    }
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let mut unified = format!("@@ -0,0 +1,{} @@\n", lines.len());
    for line in &lines {
        unified.push('+');
        unified.push_str(line);
        unified.push('\n');
    }
    let section = section(None, "—", "Working tree", path, &unified);
    doc("untracked · new file", None, vec![section], None)
}
