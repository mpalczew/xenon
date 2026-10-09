use super::*;
use serde_json::json;

#[test]
fn rejects_ssh_options_and_shell_input_as_hosts() {
    for host in ["", "-oProxyCommand=bad", "host;touch bad", "host\nother"] {
        assert!(SshWorkspace::new(host.into(), "/src".into()).is_err());
    }
    assert!(SshWorkspace::new("dev@example.org".into(), "~/src/my project".into()).is_ok());
}

#[test]
fn rejects_ambiguous_remote_roots() {
    assert!(SshWorkspace::new("box".into(), "src".into()).is_err());
    assert!(SshWorkspace::new("box".into(), "/src\nother".into()).is_err());
}

#[test]
fn remote_failures_are_not_successful_empty_files() {
    let error = transport::decode(br#"{"error":"File changed remotely; reload before saving"}"#)
        .unwrap_err();
    assert!(error.to_string().contains("File changed remotely"));
    assert_eq!(
        transport::decode(br#"{"value":null}"#).unwrap(),
        json!(null)
    );
    assert!(transport::decode(b"ssh banner").is_err());
}

#[test]
fn shell_arguments_preserve_spaces_quotes_and_substitutions() {
    assert_eq!(
        transport::quote("a'b $(touch bad)"),
        "'a'\\''b $(touch bad)'"
    );
}

#[test]
fn reconnect_reattaches_the_same_session_without_restarting_the_shell() {
    let workspace = SshWorkspace::parse("ssh://devbox/~/my project").unwrap();
    assert_eq!(workspace.directory, "~/my project");
    let first = workspace.terminal_command("xenon-fixed-session").unwrap();
    assert_eq!(
        first,
        workspace.terminal_command("xenon-fixed-session").unwrap()
    );
    assert!(first.contains("tmux -L xenon new-session -A"));
    assert!(first.contains("xenon-fixed-session"));
    assert!(first.contains("Reconnecting"));
    assert!(workspace.terminal_command("").is_err());
    assert!(workspace.terminal_command("bad/session").is_err());
}
