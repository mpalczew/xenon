use std::path::{Path, PathBuf};

use crate::{Backing, Layout, Registry, Stream, WorkspaceRec};

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
fn layout_split_ratio_is_clamped() {
    assert_eq!(
        Layout::split(0.01),
        Layout::Split {
            ratio: Layout::MIN_RATIO
        }
    );
    assert_eq!(
        Layout::split(0.99),
        Layout::Split {
            ratio: Layout::MAX_RATIO
        }
    );
    assert_eq!(Layout::split(0.5), Layout::Split { ratio: 0.5 });
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
