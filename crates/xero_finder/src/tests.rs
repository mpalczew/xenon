use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use crate::{FileIndex, Finder};

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "").unwrap();
    fs::write(root.join("src/lib.rs"), "").unwrap();
    fs::write(root.join("README.md"), "").unwrap();
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.join("ignored.txt"), "").unwrap();
    dir
}

fn finder(dir: &TempDir) -> Finder {
    Finder::new(Arc::new(FileIndex::build(dir.path())))
}

#[test]
fn walk_respects_gitignore() {
    let dir = fixture();
    // main.rs, lib.rs, README.md, .gitignore + the src/ dir; not ignored.txt.
    assert_eq!(finder(&dir).query("").len(), 5);
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
    assert_eq!(finder(&dir).query("").len(), 5);
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
