use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use crate::Finder;

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

#[test]
fn walk_respects_gitignore() {
    let dir = fixture();
    let finder = Finder::start(dir.path());
    // Files main.rs, lib.rs, README.md, .gitignore + the src/ dir; not ignored.txt.
    assert_eq!(finder.file_count(), 5);
}

#[test]
fn query_ranks_matches_first() {
    let dir = fixture();
    let mut finder = Finder::start(dir.path());
    let results = finder.query("main");
    assert_eq!(results.first().unwrap().path, PathBuf::from("src/main.rs"));
}

#[test]
fn empty_query_lists_all_files() {
    let dir = fixture();
    let mut finder = Finder::start(dir.path());
    assert_eq!(finder.query("").len(), finder.file_count());
}

#[test]
fn nonmatching_query_returns_nothing() {
    let dir = fixture();
    let mut finder = Finder::start(dir.path());
    assert!(finder.query("zzzznope").is_empty());
}
