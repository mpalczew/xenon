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
fn walk_respects_gitignore() {
    let dir = fixture();
    // main.rs, lib.rs, README.md, .gitignore, .env + the src/ dir;
    // not ignored.txt and not anything under .git.
    assert_eq!(finder(&dir).query("").len(), 6);
}

#[test]
fn walk_includes_hidden_files() {
    let dir = fixture();
    let paths: Vec<_> = finder(&dir).query("").into_iter().map(|m| m.path).collect();
    assert!(paths.contains(&PathBuf::from(".gitignore")));
    assert!(paths.contains(&PathBuf::from(".env")));
    assert!(!paths.iter().any(|p| p.starts_with(".git")));
}

#[test]
fn query_ranks_matches_first() {
    let dir = fixture();
    let results = finder(&dir).query("main");
    assert_eq!(results.first().unwrap().path, PathBuf::from("src/main.rs"));
}

#[test]
fn empty_query_lists_all_files() {
    let dir = fixture();
    assert_eq!(finder(&dir).query("").len(), 6);
}

#[test]
fn nonmatching_query_returns_nothing() {
    let dir = fixture();
    assert!(finder(&dir).query("zzzznope").is_empty());
}

#[test]
fn a_shared_index_drives_a_finder() {
    let dir = fixture();
    // The Arc<FileIndex> is what XenonApp caches and hands to each finder.
    let index = Arc::new(FileIndex::build(dir.path()));
    let mut finder = Finder::new(index);
    assert_eq!(
        finder.query("main").first().unwrap().path,
        PathBuf::from("src/main.rs")
    );
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
    let results = index.query(".claude.json");
    assert_eq!(
        results.first().map(|m| m.path.as_path()),
        Some(Path::new(".claude.json")),
        "top was {:?}, expected .claude.json",
        results.first().map(|m| &m.path)
    );
}

#[test]
fn file_index_query_is_shareable() {
    let dir = fixture();
    let index = Arc::new(FileIndex::build(dir.path()));
    let results = index.query("README");
    assert_eq!(results.first().unwrap().path, PathBuf::from("README.md"));
}

#[test]
fn walk_skips_heavy_dirs_keeps_dotfiles() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    fs::write(root.join("node_modules/pkg/index.js"), "").unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();
    fs::write(root.join("target/debug/x"), "").unwrap();
    fs::create_dir_all(root.join(".claude")).unwrap();
    fs::write(root.join(".claude.json"), "").unwrap();
    fs::write(root.join(".claude/settings.json"), "").unwrap();
    fs::write(root.join("app.rs"), "").unwrap();

    let index = FileIndex::build(root);
    let paths: Vec<_> = index
        .query("")
        .into_iter()
        .map(|m| m.path.to_string_lossy().into_owned())
        .collect();
    // empty query only returns MAX_RESULTS but fixture is tiny
    let all = {
        // re-query via full index len + known paths
        assert!(index.len() >= 3);
        index
    };
    let mut finder = Finder::new(Arc::new(all));
    // blank listing is capped; use targeted queries instead
    assert!(
        finder
            .query("app")
            .iter()
            .any(|m| m.path.ends_with("app.rs"))
    );
    assert!(
        finder
            .query(".claude.json")
            .first()
            .is_some_and(|m| m.path.as_path() == Path::new(".claude.json"))
    );
    assert!(
        finder
            .query("settings")
            .iter()
            .any(|m| m.path.ends_with(".claude/settings.json")
                || m.path.as_path() == Path::new(".claude/settings.json"))
    );
    assert!(
        !finder
            .query("index")
            .iter()
            .any(|m| { m.path.to_string_lossy().contains("node_modules") })
    );
    assert!(!paths.iter().any(|p| p.contains("node_modules")));
    assert!(
        !finder
            .query("x")
            .iter()
            .any(|m| { m.path.to_string_lossy().contains("target") })
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
