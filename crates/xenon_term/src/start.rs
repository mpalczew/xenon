use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use xenon_core::{TermId, WorkspaceId};

use crate::dir;

const STALE_SPEC: Duration = Duration::from_secs(60);
const PASS_ON: [&str; 2] = ["XENON_RUNTIME_DIR", "XENON_DATA_DIR"];

/// The spawn request, passed in a file because argv lands in `systemctl status` and
/// task env can carry secrets. A cross-build contract: fields are only added.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Spec {
    pub v: u32,
    #[serde(default)]
    pub workspace: Option<WorkspaceId>,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
}

pub(crate) fn installed_binary() -> PathBuf {
    let dir = std::env::var_os("XENON_APP_DIR").map_or_else(
        || {
            let data = std::env::var_os("XDG_DATA_HOME").map_or_else(
                || PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"),
                PathBuf::from,
            );
            data.join("xenon")
        },
        PathBuf::from,
    );
    dir.join("xenon-term")
}

pub(crate) fn slice() -> String {
    std::env::var("XENON_TERM_SLICE").unwrap_or_else(|_| "xenon-term.slice".into())
}

pub(crate) fn unit_name(id: TermId) -> String {
    format!("xenon-term-{id}.service")
}

pub(crate) fn launch(id: TermId, spec: &Spec) -> io::Result<()> {
    sweep_specs();
    write_spec(id, spec)?;
    let result = run_systemd(id);
    if result.is_err() {
        let _ = std::fs::remove_file(dir::spec_path(id));
    }
    result
}

fn write_spec(id: TermId, spec: &Spec) -> io::Result<()> {
    std::fs::create_dir_all(dir::term_dir())?;
    let path = dir::spec_path(id);
    let temp = path.with_extension("spec.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&serde_json::to_vec(spec)?)?;
    std::fs::rename(temp, path)
}

fn run_systemd(id: TermId) -> io::Result<()> {
    let mut command = Command::new("systemd-run");
    command.args([
        "--user",
        "--quiet",
        "--collect",
        "--description=Xenon terminal",
    ]);
    command.arg(format!("--unit=xenon-term-{id}"));
    command.arg(format!("--slice={}", slice()));
    // Type=notify: systemd-run returns when the holder says READY=1 (its socket is in
    // place), or fails if the holder exits first. A unit stop then behaves like closing
    // a terminal: hang up, then SIGKILL after 5 seconds.
    command.args([
        "-p",
        "Type=notify",
        "-p",
        "KillSignal=SIGHUP",
        "-p",
        "TimeoutStopSec=5",
    ]);
    for name in PASS_ON {
        if let Some(value) = std::env::var_os(name) {
            let mut pair = std::ffi::OsString::from(format!("--setenv={name}="));
            pair.push(value);
            command.arg(pair);
        }
    }
    command
        .arg("--")
        .arg(installed_binary())
        .arg("hold")
        .arg(id.to_string());
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(io::Error::other(format!("systemd-run: {text}")))
}

fn sweep_specs() {
    let Ok(entries) = std::fs::read_dir(dir::term_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| SystemTime::now().duration_since(at).ok())
            .is_some_and(|age| age > STALE_SPEC);
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if old && (name.ends_with(".spec") || name.ends_with(".spec.tmp")) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

pub(crate) fn take_spec(id: TermId) -> io::Result<Spec> {
    let path = dir::spec_path(id);
    let bytes = std::fs::read(&path)?;
    let _ = std::fs::remove_file(&path);
    Ok(serde_json::from_slice(&bytes)?)
}
