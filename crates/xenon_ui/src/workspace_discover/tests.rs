use super::*;
use std::fs;

#[test]
fn project_roots_rank_before_folders_inside_a_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("xenon/.git")).unwrap();
    fs::create_dir_all(root.join("xenon/crates/core/src")).unwrap();
    fs::create_dir_all(root.join("srcery")).unwrap();
    let mut out = Vec::new();
    walk_for_name(root, "src", &mut out);
    finish_found(&mut out);
    let names: Vec<_> = out
        .iter()
        .map(|f| f.path.strip_prefix(root).unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        [
            PathBuf::from("srcery"),
            PathBuf::from("xenon/crates/core/src")
        ]
    );
    assert!(out[1].inside_repo && !out[0].inside_repo);
}

#[test]
fn visit_cap_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    // Many siblings so BFS would be large without a cap.
    for i in 0..100 {
        fs::create_dir_all(root.join(format!("p{i}/skills"))).unwrap();
    }
    let mut out = Vec::new();
    walk_for_name(root, "skills", &mut out);
    // With many exact matches we stop early; must not hang / collect unbounded.
    assert!(out.len() <= MAX_COLLECT);
    assert!(exact_count(&out) >= 1);
}
