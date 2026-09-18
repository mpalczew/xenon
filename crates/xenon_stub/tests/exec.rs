//! Drive the shipped stub binary: temp HOME, fake `xenon-bin` that prints env.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use xenon_stub::gui_binary;

fn stub_exe() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_xenon-stub")
        .or_else(|| std::env::var_os("CARGO_BIN_EXE_xenon_stub"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!(
                "missing CARGO_BIN_EXE for xenon-stub; have {:?}",
                std::env::vars()
                    .filter(|(k, _)| k.contains("CARGO_BIN") || k.contains("CARGO_PKG"))
                    .collect::<Vec<_>>()
            )
        })
}

fn install_stub(macos: &Path) -> PathBuf {
    fs::create_dir_all(macos).unwrap();
    let stub = macos.join("xenon");
    fs::copy(stub_exe(), &stub).unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    stub
}

fn write_gui_printer(macos: &Path) {
    let gui = gui_binary(&macos.join("xenon"));
    fs::write(
        &gui,
        "#!/bin/sh\n\
         printf 'XENON_DATA_DIR=%s\\n' \"$XENON_DATA_DIR\"\n\
         printf 'XERO_DATA_DIR=%s\\n' \"$XERO_DATA_DIR\"\n\
         printf 'XENON_SLOT=%s\\n' \"$XENON_SLOT\"\n\
         printf 'XERO_SLOT=%s\\n' \"$XERO_SLOT\"\n\
         printf 'ARGS=%s\\n' \"$*\"\n",
    )
    .unwrap();
    fs::set_permissions(&gui, fs::Permissions::from_mode(0o755)).unwrap();
}

fn run_stub(stub: &Path, home: &Path, extra_env: &[(&str, &str)]) -> String {
    let mut cmd = Command::new(stub);
    cmd.env("HOME", home)
        .env_remove("XENON_DATA_DIR")
        .env_remove("XERO_DATA_DIR")
        .env_remove("XENON_SLOT")
        .env_remove("XERO_SLOT")
        .args(["open", "file.rs"]);
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    let out = cmd.output().expect("run stub");
    assert!(
        out.status.success(),
        "stub failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn stub_adopts_last_slot() {
    let root = tempfile::TempDir::new().unwrap();
    let macos = root.path().join("MacOS");
    let stub = install_stub(&macos);
    write_gui_printer(&macos);
    let home = root.path().join("home");
    fs::create_dir_all(home.join(".xenon-a")).unwrap();
    fs::write(home.join(".xenon-active-slot"), "a\n").unwrap();

    let stdout = run_stub(&stub, &home, &[]);
    let slot = home.join(".xenon-a");
    assert!(stdout.contains(&format!("XENON_DATA_DIR={}", slot.display())));
    assert!(stdout.contains(&format!("XERO_DATA_DIR={}", slot.display())));
    assert!(stdout.contains("XENON_SLOT=A"));
    assert!(stdout.contains("XERO_SLOT=A"));
    assert!(stdout.contains("ARGS=open file.rs"));
}

#[test]
fn stub_without_marker_uses_ship_dir() {
    let root = tempfile::TempDir::new().unwrap();
    let macos = root.path().join("MacOS");
    let stub = install_stub(&macos);
    write_gui_printer(&macos);
    let home = root.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let stdout = run_stub(&stub, &home, &[]);
    let ship = home.join(".xenon");
    assert!(stdout.contains(&format!("XENON_DATA_DIR={}", ship.display())));
    assert!(stdout.contains("XENON_SLOT=\n"));
}

#[test]
fn stub_preserves_existing_data_dir_env() {
    let root = tempfile::TempDir::new().unwrap();
    let macos = root.path().join("MacOS");
    let stub = install_stub(&macos);
    write_gui_printer(&macos);
    let home = root.path().join("home");
    fs::create_dir_all(home.join(".xenon-b")).unwrap();
    fs::write(home.join(".xenon-active-slot"), "b").unwrap();
    let preset = root.path().join("preset-data");
    fs::create_dir(&preset).unwrap();

    let stdout = run_stub(
        &stub,
        &home,
        &[
            ("XENON_DATA_DIR", preset.to_str().unwrap()),
            ("XENON_SLOT", "A"),
        ],
    );
    assert!(stdout.contains(&format!("XENON_DATA_DIR={}", preset.display())));
    assert!(stdout.contains("XENON_SLOT=A"));
    assert!(!stdout.contains(&format!(
        "XENON_DATA_DIR={}",
        home.join(".xenon-b").display()
    )));
}
