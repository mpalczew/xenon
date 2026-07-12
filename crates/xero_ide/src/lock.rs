//! The discovery lock file at `~/.claude/ide/<port>.lock`. Claude Code reads it
//! (found via `CLAUDE_CODE_SSE_PORT`) to learn the auth token and workspaces.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
struct Lock<'a> {
    pid: u32,
    #[serde(rename = "workspaceFolders")]
    workspace_folders: &'a [PathBuf],
    #[serde(rename = "ideName")]
    ide_name: &'a str,
    transport: &'a str,
    #[serde(rename = "runningInWindows")]
    running_in_windows: bool,
    #[serde(rename = "authToken")]
    auth_token: &'a str,
}

fn ide_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".claude").join("ide")
}

/// Write the lock file for `port` and return its path.
pub fn write(port: u16, token: &str, roots: &[PathBuf]) -> Result<PathBuf> {
    let dir = ide_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{port}.lock"));
    let lock = Lock {
        pid: std::process::id(),
        workspace_folders: roots,
        ide_name: "xenon",
        transport: "ws",
        running_in_windows: false,
        auth_token: token,
    };
    fs::write(&path, serde_json::to_vec(&lock)?)?;
    Ok(path)
}

pub fn remove(path: &Path) {
    let _ = fs::remove_file(path);
}
