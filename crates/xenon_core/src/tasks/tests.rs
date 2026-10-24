use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::*;

fn set(json: &str) -> TaskSet {
    parse_task_set(json, Os::Mac).unwrap()
}

fn ctx_run<R>(
    editor: Option<&EditorInfo>,
    inputs: &[(&str, &str)],
    f: impl FnOnce(&VarContext) -> R,
) -> R {
    let env = |k: &str| (k == "HOME_X").then(|| "/h".to_string());
    let config = BTreeMap::from([("font_size".to_string(), "14".to_string())]);
    let inputs: BTreeMap<String, String> = inputs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let secrets = BTreeSet::from(["pw".to_string()]);
    f(&VarContext {
        workspace: Path::new("/ws/app"),
        editor,
        env: &env,
        config: &config,
        inputs: &inputs,
        secret_inputs: &secrets,
        default_build_task: Some("build"),
        exec_path: "/x/xenon",
        user_home: Path::new("/Users/me"),
    })
}

#[test]
fn jsonc_os_override_and_globals() {
    let s = set(r#"{
      // comment
      "version": "2.0.0",
      "options": { "env": { "A": "1" }, "cwd": "sub" },
      "presentation": { "reveal": "silent" },
      "tasks": [
        { "label": "t", "command": "echo", "args": ["a b", {"value": "$X", "quoting": "weak"}],
          "options": { "env": { "B": "2" } },
          "osx": { "command": "say" }, "linux": { "command": "nope" },
          "runOptions": { "runOn": "folderOpen" },
          "group": { "kind": "build", "isDefault": true },
        },
      ],
    }"#);
    let t = &s.tasks[0];
    assert_eq!(t.command.as_ref().unwrap().value, "say");
    assert_eq!(t.options.env.len(), 2);
    assert_eq!(t.options.cwd.as_deref(), Some("sub"));
    assert_eq!(t.presentation.reveal, Reveal::Silent);
    assert!(t.run_on_folder_open);
    assert_eq!(s.default_of(GroupKind::Build).unwrap().label, "t");
    let resolved = ctx_run(None, &[], |c| resolve_task(t, c, "/bin/zsh")).unwrap();
    assert_eq!(resolved.program, "/bin/zsh");
    assert_eq!(resolved.args, vec!["-l", "-c", "say 'a b' \"$X\""]);
    assert_eq!(resolved.cwd, Path::new("/ws/app/sub"));
}

#[test]
fn unsupported_and_legacy() {
    let s = set(r#"{ "tasks": [
      { "label": "n", "type": "npm", "script": "x" },
      { "label": "d", "command": "c", "dependsOn": [{"type": "npm"}] },
      { "label": "p", "type": "process", "command": "ls" },
      { "command": "no label" } ] }"#);
    assert!(s.get("n").unwrap().unsupported.is_some());
    assert!(s.get("d").unwrap().unsupported.is_some());
    assert!(s.get("p").unwrap().unsupported.is_none());
    assert_eq!(s.warnings.len(), 1);
    let legacy = set(r#"{ "version": "0.1.0", "command": "make", "args": ["-j"],
      "tasks": [ { "taskName": "all", "isBuildCommand": true } ] }"#);
    let t = &legacy.tasks[0];
    assert_eq!(
        t.args.iter().map(|a| a.value.as_str()).collect::<Vec<_>>(),
        ["-j", "all"]
    );
    assert_eq!(t.group.unwrap().kind, GroupKind::Build);
}

#[test]
fn variables() {
    let ed = EditorInfo {
        file: "/ws/app/src/main.rs".into(),
        line: 7,
        selected_text: "sel".into(),
    };
    ctx_run(Some(&ed), &[("pw", "s3cret")], |c| {
        let x = |s: &str| expand(s, c).unwrap();
        assert_eq!(
            x("${relativeFile}|${fileBasenameNoExtension}|${fileExtname}"),
            "src/main.rs|main|.rs"
        );
        assert_eq!(
            x("${fileDirnameBasename} ${lineNumber} ${selectedText} ${workspaceFolderBasename}"),
            "src 7 sel app"
        );
        assert_eq!(
            x("${env:HOME_X}${env:NOPE}${config:font_size}${defaultBuildTask}"),
            "/h14build"
        );
        assert_eq!(x("${input:pw}"), "s3cret");
        assert!(expand("${nope}", c).is_err());
        assert!(expand("${config:zzz}", c).is_err());
        assert!(expand("${input:other}", c).is_err());
    });
    ctx_run(None, &[], |c| {
        assert!(expand("${file}", c).unwrap_err().contains("active editor"));
        assert_eq!(expand("${workspaceFolder}", c).unwrap(), "/ws/app");
    });
}

#[test]
fn secrets_never_in_display() {
    let s =
        set(r#"{ "tasks": [ { "label": "t", "command": "login", "args": ["${input:pw}"] } ] }"#);
    ctx_run(None, &[("pw", "s3cret")], |c| {
        let r = resolve_task(&s.tasks[0], c, "/bin/sh").unwrap();
        assert!(r.args.last().unwrap().contains("s3cret"));
        assert!(!r.display.contains("s3cret"));
        assert!(!preview_line(&s.tasks[0], c).contains("s3cret"));
    });
    assert_eq!(required_inputs(&s, &["t".into()]), ["pw"]);
}

#[test]
fn plan_order_cycles_and_failure() {
    let s = set(r#"{ "tasks": [
      { "label": "root", "command": "c", "dependsOn": ["a", "b"], "dependsOrder": "sequence" },
      { "label": "a", "command": "c" }, { "label": "b", "command": "c", "dependsOn": ["a"] } ] }"#);
    let mut o = Orchestrator::new(&s, "root").unwrap();
    assert_eq!(o.start(), [Action::Launch("a".into())]);
    assert_eq!(o.satisfied("a"), [Action::Launch("b".into())]);
    assert_eq!(o.satisfied("b"), [Action::Launch("root".into())]);
    o.satisfied("root");
    assert_eq!(o.outcome(), Some(Outcome::Succeeded));

    let parallel = set(r#"{ "tasks": [
      { "label": "root", "command": "c", "dependsOn": ["a", "b"] },
      { "label": "a", "command": "c" }, { "label": "b", "command": "c" } ] }"#);
    let mut o = Orchestrator::new(&parallel, "root").unwrap();
    assert_eq!(o.start().len(), 2);
    o.satisfied("a");
    assert!(o.failed("b", "exit 2").is_empty());
    assert!(o.satisfied("a").is_empty());
    assert!(matches!(o.outcome(), Some(Outcome::Failed { .. })));

    let cyc = set(
        r#"{ "tasks": [ { "label": "a", "command": "c", "dependsOn": "b" },
      { "label": "b", "command": "c", "dependsOn": ["a"] } ] }"#,
    );
    assert!(Orchestrator::new(&cyc, "a").unwrap_err().contains("cycle"));
    assert!(Orchestrator::new(&cyc, "zz").is_err());
}

#[test]
fn background_readiness() {
    let s = set(r#"{ "tasks": [
      { "label": "w", "command": "c", "isBackground": true, "problemMatcher": {
        "background": { "beginsPattern": "^start", "endsPattern": "^ready" } } },
      { "label": "t", "command": "c", "isBackground": true, "problemMatcher": "$tsc-watch" },
      { "label": "n", "command": "c", "isBackground": true } ] }"#);
    let mut r = Readiness::new(s.get("w").unwrap().background.as_ref());
    assert!(!r.feed(&["ready too early", "noise"]));
    assert!(!r.feed(&["start"]));
    assert!(r.feed(&["ready now"]));
    let mut r = Readiness::new(s.get("t").unwrap().background.as_ref());
    assert!(r.feed(&["12:01:02 AM - Found 0 errors. Watching for file changes."]));
    let mut r = Readiness::new(s.get("n").unwrap().background.as_ref());
    assert!(!r.feed(&["  "]));
    assert!(r.feed(&["hello"]));
}

#[test]
fn trust_hash_tracks_definitions() {
    let json = |cmd: &str| {
        format!(
            r#"{{ "tasks": [
      {{ "label": "o", "command": "{cmd}", "dependsOn": "d", "runOptions": {{ "runOn": "folderOpen" }} }},
      {{ "label": "d", "command": "dep" }}, {{ "label": "other", "command": "x" }} ] }}"#
        )
    };
    let a = folder_open_hash(&set(&json("one"))).unwrap();
    assert_eq!(a, folder_open_hash(&set(&json("one"))).unwrap());
    assert_ne!(a, folder_open_hash(&set(&json("two"))).unwrap());
    assert_eq!(folder_open_closure(&set(&json("one"))).len(), 2);
    assert!(folder_open_hash(&set(r#"{ "tasks": [] }"#)).is_none());
}
