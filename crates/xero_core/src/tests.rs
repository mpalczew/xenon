use std::path::{Path, PathBuf};

use crate::{Backing, Layout, Registry, SessionState, Stream, WorkspaceRec};

#[test]
fn workspace_name_from_root() {
    let ws = WorkspaceRec::new(PathBuf::from("/Users/me/src/xero"));
    assert_eq!(ws.name, "xero");
    assert!(ws.streams.is_empty());
}

#[test]
fn stream_working_dir_is_checkout_root() {
    let stream = Stream::new("main");
    let root = Path::new("/Users/me/src/xero");
    assert_eq!(stream.working_dir(root), root.to_path_buf());
    assert_eq!(stream.backing, Backing::Checkout);
}

#[test]
fn layout_default_is_all_visible() {
    assert_eq!(
        Layout::default(),
        Layout {
            terminal_visible: true,
            editor_visible: true,
            sidebar_visible: true,
        }
    );
}

#[test]
fn layout_round_trips_through_json() {
    let layout = Layout {
        terminal_visible: false,
        editor_visible: true,
        sidebar_visible: false,
    };
    let json = serde_json::to_string_pretty(&layout).unwrap();
    let parsed: Layout = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, layout);
}

#[test]
fn old_layout_json_defaults_all_visible() {
    let json = r#"{"editors": [], "active_editor": null, "terminal": {"cwd": "."}}"#;
    let parsed: SessionState = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.layout, Layout::default());
}

#[test]
fn registry_round_trips_through_json() {
    let mut registry = Registry::default();
    let mut ws = WorkspaceRec::new(PathBuf::from("/tmp/proj"));
    let stream = Stream::new("fix-ci");
    ws.streams.push(stream.id);
    registry.workspaces.push(ws);

    let json = serde_json::to_string_pretty(&registry).unwrap();
    let parsed: Registry = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, registry);
    assert_eq!(parsed.version, crate::CURRENT_VERSION);
}

#[test]
fn old_registry_json_defaults_closed_workspaces() {
    let json = r#"{
  "version": 1,
  "workspaces": [],
  "active": null
}"#;
    let parsed: Registry = serde_json::from_str(json).unwrap();

    assert!(parsed.closed_workspaces.is_empty());
}

#[test]
fn registry_preserves_closed_workspaces() {
    let mut registry = Registry::default();
    registry
        .closed_workspaces
        .push(WorkspaceRec::new(PathBuf::from("/tmp/closed")));

    let json = serde_json::to_string_pretty(&registry).unwrap();
    let parsed: Registry = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.closed_workspaces, registry.closed_workspaces);
}

#[test]
fn stream_round_trips_through_json() {
    let stream = Stream::new("main");
    let json = serde_json::to_string(&stream).unwrap();
    let parsed: Stream = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, stream);
}

#[test]
fn checkout_backing_serializes_with_kind_tag() {
    let json = serde_json::to_string(&Backing::Checkout).unwrap();
    assert_eq!(json, r#"{"kind":"checkout"}"#);
}
