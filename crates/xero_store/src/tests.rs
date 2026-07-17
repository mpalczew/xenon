use std::fs;

use tempfile::TempDir;
use xero_core::{Registry, SessionState, WorkspaceRec};

use crate::{
    AppSettings, StoreError, load_registry, load_session, load_settings, save_registry,
    save_session, save_settings,
};

/// Point `data_dir()` at a temp directory for the duration of a closure.
/// Serialized via a mutex since env vars are process-global.
fn with_data_dir<R>(body: impl FnOnce() -> R) -> R {
    let _guard = crate::DATA_DIR_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = TempDir::new().unwrap();
    // Prefer XENON_DATA_DIR (checked first by data_dir); clear both on exit.
    let prev_xenon = std::env::var_os("XENON_DATA_DIR");
    let prev_xero = std::env::var_os("XERO_DATA_DIR");
    unsafe {
        std::env::set_var("XENON_DATA_DIR", dir.path());
        std::env::remove_var("XERO_DATA_DIR");
    }
    let result = body();
    unsafe {
        match prev_xenon {
            Some(v) => std::env::set_var("XENON_DATA_DIR", v),
            None => std::env::remove_var("XENON_DATA_DIR"),
        }
        match prev_xero {
            Some(v) => std::env::set_var("XERO_DATA_DIR", v),
            None => std::env::remove_var("XERO_DATA_DIR"),
        }
    }
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
        registry
            .workspaces
            .push(WorkspaceRec::new("/tmp/proj".into()));
        save_registry(&registry).unwrap();
        assert_eq!(load_registry().unwrap(), registry);
    });
}

#[test]
fn registry_saves_closed_workspaces() {
    with_data_dir(|| {
        let mut registry = Registry::default();
        registry
            .closed_workspaces
            .push(WorkspaceRec::new("/tmp/closed".into()));

        save_registry(&registry).unwrap();

        assert_eq!(load_registry().unwrap(), registry);
    });
}

#[test]
fn session_saves_and_loads_by_workspace() {
    with_data_dir(|| {
        let ws = WorkspaceRec::new("/tmp/proj".into());
        let session = SessionState::default();
        save_session(ws.id, &session).unwrap();
        assert_eq!(load_session(ws.id).unwrap(), session);
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
fn settings_default_when_missing() {
    with_data_dir(|| {
        assert_eq!(load_settings().unwrap(), AppSettings::default());
    });
}

#[test]
fn settings_round_trip() {
    with_data_dir(|| {
        let settings = AppSettings {
            editor_font_size: 18.0,
            terminal_font_size: 12.0,
            ui_font_size: 15.0,
            editor_font_family: "SF Mono".into(),
            terminal_font_family: "Menlo".into(),
            ui_font_family: ".SystemUIFont".into(),
            show_line_numbers: false,
            vim_mode: true,
            theme: crate::ThemeMode::Dark,
            light_theme: "Ayu Light".into(),
            dark_theme: "Ayu Dark".into(),
            terminal_auto_close: crate::TerminalAutoClose::Immediate,
            workspaces_collapsed: true,
            files_open: false,
        };
        save_settings(&settings).unwrap();
        assert_eq!(load_settings().unwrap(), settings);
    });
}

#[test]
fn settings_missing_theme_defaults_to_system() {
    with_data_dir(|| {
        let path = crate::data_dir().join("settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"editor_font_size":16.0,"show_line_numbers":true,"vim_mode":false}"#,
        )
        .unwrap();
        let settings = load_settings().unwrap();
        assert_eq!(settings.theme, crate::ThemeMode::System);
        assert_eq!(settings.editor_font_size, 16.0);
        assert_eq!(settings.terminal_font_size, 14.0);
        assert_eq!(settings.ui_font_size, 14.0);
        assert_eq!(settings.editor_font_family, "Menlo");
        assert_eq!(settings.ui_font_family, ".SystemUIFont");
    });
}

#[test]
fn settings_migrates_legacy_font_size() {
    with_data_dir(|| {
        let path = crate::data_dir().join("settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"font_size":18.0,"show_line_numbers":true,"vim_mode":false}"#,
        )
        .unwrap();
        let settings = load_settings().unwrap();
        assert_eq!(settings.editor_font_size, 18.0);
        assert_eq!(settings.terminal_font_size, 18.0);
    });
}

#[test]
fn corrupt_settings_defaults() {
    with_data_dir(|| {
        let path = crate::data_dir().join("settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not json").unwrap();
        assert_eq!(load_settings().unwrap(), AppSettings::default());
        assert!(path.with_extension("corrupt").exists());
    });
}

#[test]
fn corrupt_session_returns_error() {
    with_data_dir(|| {
        let ws = WorkspaceRec::new("/tmp/proj".into());
        let session = SessionState::default();
        save_session(ws.id, &session).unwrap();

        let path = crate::data_dir()
            .join("sessions")
            .join(format!("{}.json", ws.id));
        fs::write(&path, "garbage").unwrap();

        assert!(matches!(
            load_session(ws.id),
            Err(StoreError::Corrupt { .. })
        ));
    });
}

#[test]
fn migrates_legacy_stream_session() {
    with_data_dir(|| {
        let ws = WorkspaceRec::new("/tmp/proj".into());
        let dir = crate::data_dir().join("streams").join(ws.id.to_string());
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("legacy.json"),
            r#"{
  "id": "00000000-0000-4000-8000-000000000099",
  "name": "main",
  "backing": {"kind": "checkout"},
  "session": {
    "layout": {
      "terminal_visible": false,
      "editor_visible": true,
      "sidebar_visible": true,
      "sidebar_width": 200.0,
      "terminal_width": 640.0
    },
    "editors": [],
    "active_editor": null,
    "terminal": {"cwd": "."}
  }
}"#,
        )
        .unwrap();

        let session = load_session(ws.id).unwrap();
        assert!(!session.layout.terminal_visible);
        assert_eq!(session.layout.sidebar_width, 200.0);
        assert!(
            crate::data_dir()
                .join("sessions")
                .join(format!("{}.json", ws.id))
                .exists()
        );
    });
}
