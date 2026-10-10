use super::*;

fn path<'a>(host: &'a str, rest: &'a str) -> Option<SshInput<'a>> {
    Some(SshInput::Path { host, rest })
}

#[test]
fn query_text_is_the_whole_state() {
    assert_eq!(SshInput::parse("src"), None);
    assert_eq!(SshInput::parse("ssh:/"), None);
    assert_eq!(SshInput::parse("ssh://"), Some(SshInput::Hosts("")));
    assert_eq!(SshInput::parse("ssh://thin"), Some(SshInput::Hosts("thin")));
    assert_eq!(SshInput::parse("ssh:///"), Some(SshInput::Hosts("")));
    assert_eq!(SshInput::parse("ssh://box/"), path("box", ""));
    assert_eq!(SshInput::parse("ssh://box/a/b"), path("box", "a/b"));
    assert_eq!(SshInput::parse("ssh://box//etc"), path("box", "/etc"));
}

#[test]
fn ss_tab_writes_the_scheme_only_for_partial_prefixes() {
    assert_eq!(scheme_completion("ss"), Some("ssh://"));
    assert_eq!(scheme_completion("ssh:/"), Some("ssh://"));
    assert_eq!(scheme_completion("s"), None);
    assert_eq!(scheme_completion("ssh://"), None);
    assert_eq!(scheme_completion("sx"), None);
}

#[test]
fn hosts_match_by_case_insensitive_prefix() {
    assert!(host_matches("thinkpad", "thin"));
    assert!(host_matches("ThinkPad", "thin"));
    assert!(host_matches("nuc", ""));
    assert!(!host_matches("thinkpad", "pad"));
    assert_eq!(host_completion("thinkpad"), "ssh://thinkpad/");
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
fn whole_folder_detection() {
    assert!(names_whole_folder(""));
    assert!(names_whole_folder("~"));
    assert!(names_whole_folder("src/"));
    assert!(!names_whole_folder("src"));
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
