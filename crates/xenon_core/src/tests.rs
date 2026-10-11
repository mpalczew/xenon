use std::path::PathBuf;

use crate::{Registry, SessionState, WorkspaceRec};

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
fn old_registry_and_workspace_json_get_defaults() {
    let json = r#"{
  "version": 1,
  "workspaces": [],
  "active": null
}"#;
    let parsed: Registry = serde_json::from_str(json).unwrap();
    assert!(parsed.closed_workspaces.is_empty());

    let ws_json =
        r#"{"id": "00000000-0000-4000-8000-000000000001", "name": "proj", "root": "/tmp/proj"}"#;
    let ws: WorkspaceRec = serde_json::from_str(ws_json).unwrap();
    assert_eq!(ws.last_opened, None);
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
