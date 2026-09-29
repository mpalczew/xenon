use std::fs;

use tempfile::TempDir;
use xenon_core::{Registry, SessionState, WorkspaceRec};

use crate::{
    AppSettings, StoreError, WindowGeometry, WindowState, load_registry, load_session,
    load_settings, save_registry, save_session, save_settings, update_settings,
};

/// Point `data_dir()` at a temp directory for the duration of a closure.
/// Serialized via a mutex since env vars are process-global.
fn with_data_dir<R>(body: impl FnOnce() -> R) -> R {
    let dir = TempDir::new().unwrap();
    with_data_dir_at(dir.path(), body)
}

/// Point the store at `dir` with no inherited shared dir (a shell inside a
/// Xenon slot exports XENON_SHARED_DIR; tests must never write there).
fn with_data_dir_at<R>(dir: &std::path::Path, body: impl FnOnce() -> R) -> R {
    let _guard = crate::DATA_DIR_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    const VARS: [&str; 3] = ["XENON_DATA_DIR", "XERO_DATA_DIR", "XENON_SHARED_DIR"];
    let prev: Vec<_> = VARS.iter().map(std::env::var_os).collect();
    // SAFETY: DATA_DIR_TEST_LOCK serializes every env mutation in these tests.
    unsafe {
        std::env::set_var("XENON_DATA_DIR", dir);
        std::env::remove_var("XERO_DATA_DIR");
        std::env::remove_var("XENON_SHARED_DIR");
    }
    let result = body();
    unsafe {
        for (var, value) in VARS.iter().zip(prev) {
            match value {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
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
            wrap_prose: false,
            wrap_code: true,
            vim_mode: true,
            theme: crate::ThemeMode::Dark,
            light_theme: "Ayu Light".into(),
            dark_theme: "Ayu Dark".into(),
            terminal_auto_close: crate::TerminalAutoClose::Immediate,
            workspaces_collapsed: true,
            files_open: false,
            sidebar_width: 360.0,
            workspaces_section_height: Some(180.0),
            window: Some(WindowGeometry::new(
                120.0,
                80.0,
                1400.0,
                900.0,
                WindowState::Maximized,
            )),
            remote_enabled: true,
            remote_network: crate::RemoteNetwork::TailscaleAndLan,
            remote_keep_awake: false,
            remote_port: 17890,
            remote_hostname: "macbook.tailnet.ts.net".into(),
            lsp: crate::LspSettings::default(),
        };
        save_settings(&settings).unwrap();
        assert_eq!(load_settings().unwrap(), settings);
    });
}

#[test]
fn settings_update_preserves_unowned_fields() {
    with_data_dir(|| {
        let settings = AppSettings {
            remote_hostname: "keep-me".into(),
            lsp: crate::LspSettings {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        save_settings(&settings).unwrap();

        update_settings(|settings| settings.show_line_numbers = false).unwrap();

        let updated = load_settings().unwrap();
        assert!(!updated.show_line_numbers);
        assert_eq!(updated.remote_hostname, "keep-me");
        assert!(!updated.lsp.enabled);
    });
}

#[test]
fn settings_missing_window_defaults_to_none() {
    with_data_dir(|| {
        let path = crate::data_dir().join("settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"editor_font_size":16.0,"show_line_numbers":true,"vim_mode":false}"#,
        )
        .unwrap();
        let settings = load_settings().unwrap();
        assert!(settings.window.is_none());
    });
}

#[test]
fn window_geometry_sane_rejects_tiny_or_non_finite() {
    assert!(WindowGeometry::new(0., 0., 800., 600., WindowState::Windowed).is_sane());
    assert!(!WindowGeometry::new(0., 0., 100., 600., WindowState::Windowed).is_sane());
    assert!(!WindowGeometry::new(0., 0., 800., f32::NAN, WindowState::Windowed).is_sane());
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
        assert_eq!(settings.editor_font_family, "Lilex");
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
fn load_registry_migrates_legacy_stream_session() {
    with_data_dir(|| {
        let ws = WorkspaceRec::new("/tmp/proj".into());
        let mut registry = Registry::default();
        registry.workspaces.push(ws.clone());
        save_registry(&registry).unwrap();

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

        let _ = load_registry().unwrap();
        let session = load_session(ws.id).unwrap();
        assert!(session.sidebar_visible);
        assert_eq!(session.sidebar_width, 200.0);
        // terminal_visible false + no editors → content may be empty or term-only after migrate
        assert!(session.content.is_empty() || session.content.leaf_ids().len() <= 1);
    });
}

#[test]
fn remote_devices_round_trip_owner_only() {
    with_data_dir(|| {
        assert!(crate::load_remote_devices().unwrap().devices.is_empty());
        let devices = crate::RemoteDevices {
            devices: vec![crate::RemoteDevice {
                id: "d1".into(),
                label: "iPhone · Safari".into(),
                token_sha256: "ab".into(),
                created_at: 1,
                last_seen_at: 2,
            }],
        };
        crate::save_remote_devices(&devices).unwrap();
        assert_eq!(crate::load_remote_devices().unwrap(), devices);
        let path = crate::data_dir().join("remote_devices.json");
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    });
}

#[test]
fn legacy_remote_password_is_ignored() {
    with_data_dir(|| {
        let path = crate::data_dir().join("settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"remote_password":"old","remote_port":17890}"#).unwrap();
        let settings = load_settings().unwrap();
        assert!(!settings.remote_enabled);
        assert!(settings.remote_keep_awake);
        assert_eq!(settings.remote_network, crate::RemoteNetwork::Tailscale);
    });
}

#[test]
fn slot_dirs_share_remote_devices() {
    let home = tempfile::tempdir().unwrap();
    let devices = crate::RemoteDevices {
        devices: vec![crate::RemoteDevice {
            id: "d1".into(),
            label: "iPhone".into(),
            token_sha256: "ab".into(),
            created_at: 1,
            last_seen_at: 1,
        }],
    };
    with_data_dir_at(&home.path().join(".xenon-a"), || {
        assert_eq!(crate::shared_dir(), home.path().join(".xenon-shared"));
        crate::save_remote_devices(&devices).unwrap();
    });
    with_data_dir_at(&home.path().join(".xenon-b"), || {
        assert_eq!(crate::load_remote_devices().unwrap(), devices);
    });
}

#[test]
fn per_slot_devices_file_migrates_to_shared() {
    let home = tempfile::tempdir().unwrap();
    let slot_a = home.path().join(".xenon-a");
    fs::create_dir_all(&slot_a).unwrap();
    fs::write(
        slot_a.join("remote_devices.json"),
        r#"{"devices":[{"id":"old","label":"iPhone","token_sha256":"x","created_at":1,"last_seen_at":1}]}"#,
    )
    .unwrap();
    with_data_dir_at(&slot_a, || {
        assert_eq!(crate::load_remote_devices().unwrap().devices[0].id, "old");
    });
}
