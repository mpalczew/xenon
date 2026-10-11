use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::TempDir;

use crate::{FileIndex, Finder};

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join(".git/objects")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();
    fs::write(root.join("src/lib.rs"), "").unwrap();
    fs::write(root.join("README.md"), "").unwrap();
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.join(".env"), "").unwrap();
    fs::write(root.join("ignored.txt"), "").unwrap();
    fs::write(root.join(".git/config"), "").unwrap();
    dir
}

fn finder(dir: &TempDir) -> Finder {
    Finder::new(Arc::new(FileIndex::build(dir.path())))
}

#[test]
fn walk_keeps_dotfiles_and_respects_ignores() {
    let dir = fixture();
    let paths: Vec<_> = finder(&dir)
        .query("", &[])
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert!(paths.contains(&PathBuf::from(".gitignore")));
    assert!(paths.contains(&PathBuf::from(".env")));
    assert!(!paths.iter().any(|p| p.starts_with(".git/")));
    assert!(!paths.contains(&PathBuf::from("ignored.txt")), "gitignored");
}

#[test]
fn exact_basename_beats_nested_fuzzy_match() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".claude/backups")).unwrap();
    fs::write(root.join(".claude.json"), "").unwrap();
    fs::write(root.join(".claude/backups/.claude.json.backup.123"), "").unwrap();
    fs::write(root.join(".claude/settings.json"), "").unwrap();

    let index = FileIndex::build(root);
    let results = index.query(".claude.json", &[]);
    assert_eq!(
        results.first().map(|m| m.path.as_path()),
        Some(Path::new(".claude.json")),
        "top was {:?}, expected .claude.json",
        results.first().map(|m| &m.path)
    );
}

#[test]
fn progressive_build_emits_before_done() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for i in 0..50 {
        fs::write(root.join(format!("f{i}.txt")), "").unwrap();
    }
    let mut emits = 0usize;
    let index = FileIndex::build_with_progress(root, |_| {
        emits += 1;
        true
    });
    assert!(emits >= 2, "expected partial + final, got {emits}");
    assert_eq!(index.len(), 50);
}

#[test]
fn recents_break_ties_among_equal_basename_matches() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("a")).unwrap();
    fs::create_dir_all(root.join("b")).unwrap();
    fs::write(root.join("a/util.rs"), "").unwrap();
    fs::write(root.join("b/util.rs"), "").unwrap();

    let index = FileIndex::build(root);
    // Without recents, order is path heuristics (a before b).
    let plain = index.query("util.rs", &[]);
    assert!(
        plain.iter().any(|m| m.path == *"a/util.rs")
            && plain.iter().any(|m| m.path == *"b/util.rs")
    );

    let recents = [PathBuf::from("b/util.rs")];
    let ranked = index.query("util.rs", &recents);
    assert_eq!(
        ranked.first().map(|m| m.path.as_path()),
        Some(Path::new("b/util.rs")),
        "recently opened equal basename should win, got {:?}",
        ranked.first().map(|m| &m.path)
    );
}

#[test]
fn empty_query_lists_recents_first() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::write(root.join("alpha.txt"), "").unwrap();
    fs::write(root.join("beta.txt"), "").unwrap();
    fs::write(root.join("gamma.txt"), "").unwrap();

    let index = FileIndex::build(root);
    let recents = [PathBuf::from("gamma.txt"), PathBuf::from("alpha.txt")];
    let results = index.query("", &recents);
    assert!(results.len() >= 3);
    assert_eq!(results[0].path, PathBuf::from("gamma.txt"));
    assert_eq!(results[1].path, PathBuf::from("alpha.txt"));
}

#[test]
fn exact_basename_still_beats_recent_weaker_match() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("main.rs"), "").unwrap();
    fs::write(root.join("nested/main_helper.rs"), "").unwrap();

    let index = FileIndex::build(root);
    let recents = [PathBuf::from("nested/main_helper.rs")];
    let results = index.query("main.rs", &recents);
    assert_eq!(
        results.first().map(|m| m.path.as_path()),
        Some(Path::new("main.rs")),
        "exact basename must still beat a recent weaker match"
    );
}
