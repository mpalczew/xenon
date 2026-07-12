use std::fs;
use std::path::PathBuf;
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
    // The Arc<FileIndex> is what XeroApp caches and hands to each finder.
    let index = Arc::new(FileIndex::build(dir.path()));
    let mut finder = Finder::new(index);
    assert_eq!(
        finder.query("main").first().unwrap().path,
        PathBuf::from("src/main.rs")
    );
}
