use super::*;

fn dirs(paths: &[&str]) -> Vec<RemoteDir> {
    paths
        .iter()
        .map(|p| RemoteDir {
            path: (*p).into(),
            git: false,
        })
        .collect()
}

fn listing(listed: &[&str], found: &[&str]) -> PathListing {
    PathListing {
        home: "/home/me".into(),
        listed: dirs(listed),
        found: dirs(found),
        discovery: Discovery::Done,
    }
}

fn recent(dir: &str) -> (WorkspaceCandidate, String) {
    let candidate = WorkspaceCandidate::Path {
        root: PathBuf::from(dir),
        found: true,
    };
    (candidate, dir.into())
}

fn display(row: &WorkspaceCandidate) -> &str {
    match row {
        WorkspaceCandidate::Remote { display, .. } => display,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn hosts_merge_config_with_used_most_recent_first() {
    let rows = host_candidates(
        vec!["nuc".into(), "thinkpad".into()],
        &[
            ("thinkpad".into(), 5),
            ("devbox".into(), 9),
            ("thinkpad".into(), 7),
        ],
    );
    let names: Vec<_> = rows.iter().map(|r| (r.name(), r.last_opened())).collect();
    assert_eq!(
        names,
        [
            ("ssh://thinkpad/".into(), 7),
            ("ssh://devbox/".into(), 9),
            ("ssh://nuc/".into(), 0)
        ]
    );
    assert!(
        rows.iter()
            .all(|r| r.badge() == Some("SSH") && r.selectable())
    );
}

#[test]
fn listed_folders_come_before_name_matches_without_duplicates() {
    let found = listing(
        &["/home/me/src", "/home/me/sre"],
        &["/home/me/src", "/home/me/dev/sr-old"],
    );
    let rows = merge_path("box", Vec::new(), Some(&found));
    let shown: Vec<_> = rows.iter().map(display).collect();
    assert_eq!(shown, ["~/src", "~/sre", "~/dev/sr-old"]);
}

#[test]
fn recents_lead_and_are_not_repeated() {
    let found = listing(&["/home/me/src/a", "/home/me/src/b"], &[]);
    let rows = merge_path("box", vec![recent("/home/me/src/b")], Some(&found));
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[0], WorkspaceCandidate::Path { .. }));
    assert_eq!(display(&rows[1]), "~/src/a");
}

#[test]
fn recents_saved_with_a_tilde_match_listed_absolute_paths() {
    let found = listing(&["/home/me/src/a"], &[]);
    let rows = merge_path("box", vec![recent("~/src/a")], Some(&found));
    assert_eq!(rows.len(), 1);
}

#[test]
fn recents_the_host_did_not_return_are_dropped_but_kept_while_waiting() {
    let found = listing(&["/home/me/src/a"], &[]);
    let rows = merge_path("box", vec![recent("/elsewhere/b")], Some(&found));
    assert_eq!(rows.len(), 1);
    assert!(matches!(rows[0], WorkspaceCandidate::Remote { .. }));
    let waiting = merge_path("box", vec![recent("/elsewhere/b")], None);
    assert_eq!(waiting.len(), 1);
}

#[test]
fn errors_are_one_bounded_line() {
    assert_eq!(
        short_error("SSH: Permission denied (publickey).\nmore"),
        "Permission denied (publickey)."
    );
    assert_eq!(short_error("  \n"), "connection failed");
    assert!(short_error(&"x".repeat(200)).chars().count() <= 91);
}

fn answer(paths: &[&str]) -> RemoteListing {
    RemoteListing {
        home: "/home/me".into(),
        dirs: dirs(paths),
    }
}

fn listed(token: u64, paths: &[&str], searching: bool) -> Reply {
    let body = ReplyBody::Listed {
        listing: Ok(answer(paths)),
        searching,
    };
    Reply { token, body }
}

fn discovered(token: u64, result: Result<RemoteListing, String>) -> Reply {
    let body = ReplyBody::Discovered(result);
    Reply { token, body }
}

fn ready(remote: &Remote) -> &PathListing {
    match remote {
        Remote::Ready(listing) => listing,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn the_listing_shows_while_discovery_is_pending_then_discovery_joins() {
    let remote = Remote::Searching.apply(3, listed(3, &["/home/me/src"], true));
    assert!(remote.waiting() && !remote.complete());
    assert_eq!(ready(&remote).discovery, Discovery::Pending);
    let rows = merge_path("box", Vec::new(), Some(ready(&remote)));
    assert_eq!(rows.len(), 1);

    let remote = remote.apply(3, discovered(3, Ok(answer(&["/home/me/dev/sr-old"]))));
    assert!(remote.complete() && !remote.waiting());
    let rows = merge_path("box", Vec::new(), Some(ready(&remote)));
    let shown: Vec<_> = rows.iter().map(display).collect();
    assert_eq!(shown, ["~/src", "~/dev/sr-old"]);
}

#[test]
fn without_a_search_the_listing_is_complete() {
    let remote = Remote::Searching.apply(1, listed(1, &["/home/me/src"], false));
    assert!(remote.complete());
}

#[test]
fn stale_replies_are_dropped() {
    let before = Remote::Searching.apply(2, listed(1, &["/old"], true));
    assert!(matches!(before, Remote::Searching));
    let current = before.apply(2, listed(2, &["/home/me/src"], true));
    let current = current.apply(2, discovered(1, Ok(answer(&["/old"]))));
    assert!(ready(&current).found.is_empty());
    assert_eq!(ready(&current).discovery, Discovery::Pending);
}

#[test]
fn a_late_listing_replaces_nothing_it_should_not() {
    let remote = Remote::Idle.apply(5, discovered(5, Ok(answer(&["/x"]))));
    assert!(matches!(remote, Remote::Idle));
}

#[test]
fn discovery_failure_keeps_the_listing_and_reports_below() {
    let remote = Remote::Searching.apply(1, listed(1, &["/home/me/src"], true));
    let remote = remote.apply(1, discovered(1, Err("timed out".into())));
    assert_eq!(
        ready(&remote).discovery,
        Discovery::Failed("timed out".into())
    );
    assert_eq!(ready(&remote).listed.len(), 1);
    assert!(remote.waiting() && !remote.complete());
}

#[test]
fn listing_failure_fails_the_whole_lookup() {
    let reply = Reply {
        token: 1,
        body: ReplyBody::Listed {
            listing: Err("denied".into()),
            searching: true,
        },
    };
    let remote = Remote::Searching.apply(1, reply);
    assert!(matches!(remote, Remote::Failed(ref e) if e == "denied"));
    let remote = remote.apply(1, discovered(1, Ok(answer(&["/x"]))));
    assert!(matches!(remote, Remote::Failed(_)));
}

#[test]
fn recents_survive_until_discovery_finishes() {
    let mut pending = listing(&["/home/me/src/a"], &[]);
    pending.discovery = Discovery::Pending;
    let rows = merge_path("box", vec![recent("/elsewhere/b")], Some(&pending));
    assert_eq!(rows.len(), 2);
}
