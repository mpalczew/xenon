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
    const VARS: [&str; 2] = ["XENON_DATA_DIR", "XENON_SHARED_DIR"];
    let prev: Vec<_> = VARS.iter().map(std::env::var_os).collect();
    // SAFETY: DATA_DIR_TEST_LOCK serializes every env mutation in these tests.
    unsafe {
        std::env::set_var("XENON_DATA_DIR", dir);
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
            git_collapsed: true,
            git_section_height: Some(120.0),
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
            remote_keep_awake_while_connected: true,
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
        assert!(settings.window.is_none());
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
fn task_trust_is_keyed_to_the_hash() {
    with_data_dir(|| {
        let root = std::path::Path::new("/w/app");
        assert_eq!(crate::task_trust(root, "h1"), crate::TaskTrust::Unknown);
        crate::set_task_trust(root, "h1", true).unwrap();
        assert_eq!(crate::task_trust(root, "h1"), crate::TaskTrust::Allowed);
        assert_eq!(crate::task_trust(root, "h2"), crate::TaskTrust::Unknown);
        crate::set_task_trust(root, "h2", false).unwrap();
        assert_eq!(crate::task_trust(root, "h2"), crate::TaskTrust::Denied);
        assert_eq!(crate::task_trust(root, "h1"), crate::TaskTrust::Unknown);
    });
}

#[test]
fn legacy_lan_network_setting_reads_as_tailscale_only() {
    with_data_dir(|| {
        let path = crate::data_dir().join("settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{ "remote_enabled": true, "remote_network": "tailscale_and_lan" }"#,
        )
        .unwrap();
        let settings = load_settings().unwrap();
        assert!(settings.remote_enabled);
        assert!(!path.with_extension("corrupt").exists());
    });
}
