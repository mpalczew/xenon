use super::*;

#[test]
fn scheme_prefix_is_a_proper_nonempty_prefix() {
    for query in ["s", "ss", "SSH", "ssh:/", " ssh:"] {
        assert!(scheme_prefix(query), "{query}");
    }
    for query in ["", "ssh://", "sx", "ssh://box", "sshh"] {
        assert!(!scheme_prefix(query), "{query}");
    }
}

#[test]
fn lookups_split_parent_from_last_segment() {
    let dirs = |dir: &str, prefix: &str| HostQuery::Dirs {
        dir: dir.into(),
        prefix: prefix.into(),
    };
    let name = |n: &str| Some(HostQuery::Name(n.into()));
    assert_eq!(lookups(""), (dirs("~", ""), None));
    assert_eq!(lookups("~"), (dirs("~", ""), None));
    assert_eq!(lookups("sr"), (dirs("~", "sr"), name("sr")));
    assert_eq!(lookups("src/"), (dirs("src", ""), None));
    assert_eq!(lookups("src/xe"), (dirs("src", "xe"), name("xe")));
    assert_eq!(lookups("~/src/xe"), (dirs("~/src", "xe"), name("xe")));
    assert_eq!(lookups("/"), (dirs("/", ""), None));
    assert_eq!(lookups("/e"), (dirs("/", "e"), name("e")));
    assert_eq!(lookups("/etc/ng"), (dirs("/etc", "ng"), name("ng")));
    assert_eq!(last_segment("a/b/c"), "c");
}

#[test]
fn open_address_reads_relative_absolute_and_home_paths() {
    assert_eq!(open_address("box", "src"), "ssh://box/~/src");
    assert_eq!(open_address("box", "src/"), "ssh://box/~/src");
    assert_eq!(open_address("box", "~/src"), "ssh://box/~/src");
    assert_eq!(open_address("box", "~"), "ssh://box/~/");
    assert_eq!(open_address("box", ""), "ssh://box/~/");
    assert_eq!(open_address("box", "/etc/"), "ssh://box/etc");
    assert_eq!(open_address("box", "/"), "ssh://box/");
    assert_eq!(folder_address("box", "/home/me/x"), "ssh://box/home/me/x");
}

#[test]
fn absolute_resolves_each_style() {
    assert_eq!(absolute("src", "/home/me"), "/home/me/src");
    assert_eq!(absolute("~/src", "/home/me/"), "/home/me/src");
    assert_eq!(absolute("~", "/home/me"), "/home/me");
    assert_eq!(absolute("/etc", "/home/me"), "/etc");
}

#[test]
fn tab_completes_in_the_style_the_user_typed() {
    let home = "/home/me";
    let src = "/home/me/src";
    assert_eq!(path_completion("box", "sr", src, home), "ssh://box/src/");
    assert_eq!(path_completion("box", "~/s", src, home), "ssh://box/~/src/");
    assert_eq!(
        path_completion("box", "/ho", src, home),
        "ssh://box//home/me/src/"
    );
    assert_eq!(path_completion("box", "e", "/etc", home), "ssh://box//etc/");
    assert_eq!(path_completion("box", "", home, home), "ssh://box/~/");
    assert_eq!(path_completion("box", "/", "/", home), "ssh://box//");
    assert_eq!(path_completion("box", "~", src, home), "ssh://box/~/src/");
}

#[test]
fn tab_on_a_past_workspace_without_a_listing_uses_its_tilde_path() {
    assert_eq!(path_completion("box", "x", "~/x", ""), "ssh://box/x/");
}
