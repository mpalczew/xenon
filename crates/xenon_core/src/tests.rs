use std::path::PathBuf;

use crate::{Registry, WorkspaceRec};

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
