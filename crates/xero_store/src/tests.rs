use std::fs;

use tempfile::TempDir;
use xero_core::{Registry, Stream, WorkspaceRec};

use crate::{StoreError, load_registry, load_session, save_registry, save_session};

/// Point `data_dir()` at a temp directory for the duration of a closure.
/// Serialized via a mutex since env vars are process-global.
fn with_data_dir<R>(body: impl FnOnce() -> R) -> R {
    use std::sync::Mutex;
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap();
    let dir = TempDir::new().unwrap();
    unsafe { std::env::set_var("XERO_DATA_DIR", dir.path()) };
    let result = body();
    unsafe { std::env::remove_var("XERO_DATA_DIR") };
    result
}

#[test]
fn missing_registry_loads_default() {
    with_data_dir(|| {
        let registry = load_registry().unwrap();
        assert_eq!(registry, Registry::default());
    });
}

#[test]
fn registry_saves_and_loads() {
    with_data_dir(|| {
        let mut registry = Registry::default();
        registry.workspaces.push(WorkspaceRec::new("/tmp/proj".into()));
        save_registry(&registry).unwrap();
        assert_eq!(load_registry().unwrap(), registry);
    });
}

#[test]
fn session_saves_and_loads_by_id() {
    with_data_dir(|| {
        let ws = WorkspaceRec::new("/tmp/proj".into());
        let stream = Stream::new("main");
        save_session(ws.id, &stream).unwrap();
        assert_eq!(load_session(ws.id, stream.id).unwrap(), stream);
    });
}

#[test]
fn corrupt_registry_backs_up_and_defaults() {
    with_data_dir(|| {
        let path = crate::data_dir().join("workspaces.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not valid json").unwrap();

        let registry = load_registry().unwrap();
        assert_eq!(registry, Registry::default());
        assert!(path.with_extension("corrupt").exists());
    });
}

#[test]
fn corrupt_session_returns_error() {
    with_data_dir(|| {
        let ws = WorkspaceRec::new("/tmp/proj".into());
        let stream = Stream::new("main");
        save_session(ws.id, &stream).unwrap();

        let path = crate::data_dir()
            .join("streams")
            .join(ws.id.to_string())
            .join(format!("{}.json", stream.id));
        fs::write(&path, "garbage").unwrap();

        assert!(matches!(
            load_session(ws.id, stream.id),
            Err(StoreError::Corrupt { .. })
        ));
    });
}
