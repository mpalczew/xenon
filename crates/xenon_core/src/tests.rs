use std::path::PathBuf;

use crate::{Layout, Registry, SessionState, WorkspaceRec};

#[test]
fn workspace_name_from_root() {
    let ws = WorkspaceRec::new(PathBuf::from("/Users/me/src/xero"));
    assert_eq!(ws.name, "xero");
}

#[test]
fn layout_default_is_all_visible() {
    let d = Layout::default();
    assert!(d.terminal_visible && d.editor_visible && d.sidebar_visible);
    assert_eq!(d.sidebar_width, crate::session::DEFAULT_SIDEBAR_WIDTH);
    assert_eq!(d.terminal_width, crate::session::DEFAULT_TERMINAL_WIDTH);
}

#[test]
fn layout_round_trips_through_json() {
    let layout = Layout {
        terminal_visible: false,
        editor_visible: true,
        sidebar_visible: false,
        sidebar_width: 200.,
        terminal_width: 640.,
    };
    let json = serde_json::to_string_pretty(&layout).unwrap();
    let parsed: Layout = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, layout);
}

#[test]
fn old_session_json_migrates_to_content() {
    let json = r#"{"editors": [], "active_editor": null, "terminal": {"cwd": "."}}"#;
    let parsed: SessionState = serde_json::from_str(json).unwrap();
    assert!(parsed.sidebar_visible);
    // Empty editors + default layout → single terminal leaf
    assert!(!parsed.content.is_empty());
    assert_eq!(parsed.content.leaf_ids().len(), 1);
}

#[test]
fn old_session_with_editors_and_split() {
    let json = r#"{
        "layout": {
            "terminal_visible": true,
            "editor_visible": true,
            "sidebar_visible": true,
            "sidebar_width": 220.0,
            "terminal_width": 500.0
        },
        "editors": [{"path": "src/main.rs", "cursor": {"row": 1, "col": 2}, "scroll_top": 0}],
        "active_editor": 0,
        "terminal": {"cwd": "."}
    }"#;
    let parsed: SessionState = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.sidebar_width, 220.0);
    assert_eq!(parsed.content.leaf_ids().len(), 2);
}

#[test]
fn new_session_round_trips() {
    let session = SessionState::default();
    let json = serde_json::to_string(&session).unwrap();
    let parsed: SessionState = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, session);
}

#[test]
fn registry_round_trips_through_json() {
    let mut registry = Registry::default();
    let ws = WorkspaceRec::new(PathBuf::from("/tmp/proj"));
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
fn registry_ignores_legacy_streams_field() {
    let json = r#"{
  "version": 1,
  "workspaces": [{
    "id": "00000000-0000-4000-8000-000000000001",
    "name": "proj",
    "root": "/tmp/proj",
    "streams": ["00000000-0000-4000-8000-000000000002"]
  }],
  "active": {
    "workspace": "00000000-0000-4000-8000-000000000001",
    "stream": "00000000-0000-4000-8000-000000000002"
  }
}"#;
    let parsed: Registry = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.workspaces.len(), 1);
    assert_eq!(parsed.workspaces[0].name, "proj");
    assert!(parsed.active.is_some());
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
