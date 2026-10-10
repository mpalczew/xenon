use super::*;
use std::fs;

fn tmp_tree() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    // Noise: deep contains match
    let deep = root.join("aaa/bbb/ccc/myskills-helper");
    fs::create_dir_all(&deep).unwrap();
    // Prefix match at shallow depth
    let prefix = root.join("skills-lab");
    fs::create_dir_all(&prefix).unwrap();
    // Exact match, short path, with .git — should win
    let exact = root.join("agentic/skills");
    fs::create_dir_all(&exact).unwrap();
    fs::create_dir_all(exact.join(".git")).unwrap();
    // Exact match deeper
    let deep_exact = root.join("vendorish/nested/skills");
    fs::create_dir_all(&deep_exact).unwrap();
    (tmp, root)
}

#[test]
fn match_quality_prefers_exact() {
    assert_eq!(
        MatchQuality::of_basename("skills", "skills"),
        Some(MatchQuality::Exact)
    );
    assert_eq!(
        MatchQuality::of_basename("skills-lab", "skills"),
        Some(MatchQuality::Prefix)
    );
    assert_eq!(
        MatchQuality::of_basename("myskills", "skills"),
        Some(MatchQuality::Contains)
    );
    assert_eq!(MatchQuality::of_basename("other", "skills"), None);
}

#[test]
fn sort_prefers_exact_then_short_path() {
    let (_tmp, root) = tmp_tree();
    let mut out = Vec::new();
    walk_for_name(&root, "skills", &mut out);
    finish_found(&mut out);
    assert!(!out.is_empty());
    let top = &out[0];
    assert_eq!(top.quality, MatchQuality::Exact);
    assert!(
        top.path.ends_with("agentic/skills"),
        "top was {:?}, expected agentic/skills",
        top.path
    );
    let first_non_exact = out.iter().position(|f| f.quality != MatchQuality::Exact);
    if let Some(i) = first_non_exact {
        assert!(out[..i].iter().all(|f| f.quality == MatchQuality::Exact));
    }
}

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
