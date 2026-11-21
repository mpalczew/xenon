//! Starting a holder: the spec file, then `systemd-run`. The holder is a transient unit
//! of its own, so it lives in no process's cgroup: stopping the window or the phone
//! server never takes a terminal with it.

use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use xenon_core::{TermId, WorkspaceId};

use crate::dir;

/// A spec this old belongs to an `open` that died before its holder started.
const STALE_SPEC: Duration = Duration::from_secs(60);
/// Variables a private run sets and its holders must see too.
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
    /// The whole environment, `KEY=VALUE`: the base, then the spawn's own.
    #[serde(default)]
    pub env: Vec<String>,
}

/// The one function that names the binary a new holder starts from. `open` and the
/// "older build" check both call it.
pub fn installed_binary() -> PathBuf {
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

/// The slice every holder runs in; `XENON_TERM_SLICE` names a private one for checks.
pub(crate) fn slice() -> String {
    std::env::var("XENON_TERM_SLICE").unwrap_or_else(|_| "xenon-term.slice".into())
}

pub(crate) fn unit_name(id: TermId) -> String {
    format!("xenon-term-{id}.service")
}

/// Write the spec, start the holder and return once it says it is ready. On failure the
/// spec is removed.
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

/// The holder's side: read the spec and delete it. A holder without a spec has nothing
/// to run and exits at once.
pub(crate) fn take_spec(id: TermId) -> io::Result<Spec> {
    let path = dir::spec_path(id);
    let bytes = std::fs::read(&path)?;
    let _ = std::fs::remove_file(&path);
    Ok(serde_json::from_slice(&bytes)?)
}

/// The window's environment for terminals the phone makes.
pub(crate) fn write_base_env(env: &[String]) -> io::Result<()> {
    std::fs::create_dir_all(dir::runtime_dir())?;
    let path = dir::base_env_path();
    let temp = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&serde_json::to_vec(env)?)?;
    std::fs::rename(temp, path)
}

/// The window's environment, else this process's own (the user manager's, for the phone
/// server), without what systemd set for this process alone.
pub(crate) fn base_env() -> Vec<String> {
    let saved = std::fs::read(dir::base_env_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    saved.unwrap_or_else(|| {
        std::env::vars()
            .filter(|(k, _)| !k.starts_with("LISTEN_") && k != "NOTIFY_SOCKET")
            .map(|(k, v)| format!("{k}={v}"))
            .collect()
    })
}
