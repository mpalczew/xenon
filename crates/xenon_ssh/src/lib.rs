//! SSH workspace transport. Blocking operations belong on a background executor.

mod file;
mod transport;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshWorkspace {
    pub host: String,
    pub directory: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteSnapshot {
    pub text: String,
    pub revision: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RemoteRead {
    File { snapshot: RemoteSnapshot },
    Unchanged,
    Deleted,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteEntry {
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
}

#[derive(Clone)]
pub struct RemoteFile {
    pub workspace: SshWorkspace,
    pub relative: String,
}

impl SshWorkspace {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value
            .strip_prefix("ssh://")
            .ok_or_else(|| anyhow::anyhow!("Use ssh://host/absolute/path"))?;
        let (host, directory) = value
            .split_once('/')
            .ok_or_else(|| anyhow::anyhow!("Include the remote directory"))?;
        let directory = if directory.starts_with("~/") {
            directory.to_string()
        } else {
            format!("/{directory}")
        };
        Self::new(host.to_string(), directory)
    }
    pub fn new(host: String, directory: String) -> Result<Self> {
        ensure!(
            !host.is_empty()
                && !host.starts_with('-')
                && host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"@._-:[]".contains(&b)),
            "Use an SSH host alias or user@host"
        );
        ensure!(
            directory.starts_with('/') || directory.starts_with("~/"),
            "Use an absolute remote path or ~/path"
        );
        ensure!(
            !directory.contains(['\0', '\n', '\r']),
            "Invalid remote directory"
        );
        Ok(Self { host, directory })
    }

    pub fn probe(&self) -> Result<String> {
        let value = self.request(json!({"op": "probe"}))?;
        Ok(serde_json::from_value(value)?)
    }

    pub fn entries(&self) -> Result<Vec<RemoteEntry>> {
        Ok(serde_json::from_value(
            self.request(json!({"op": "index"}))?,
        )?)
    }

    pub fn file(&self, relative: &Path) -> RemoteFile {
        RemoteFile {
            workspace: self.clone(),
            relative: relative.to_string_lossy().into_owned(),
        }
    }

    /// A stable session name survives app restarts and SSH disconnects.
    pub fn terminal_command(&self, session: &str) -> Result<String> {
        ensure!(
            !session.is_empty()
                && session
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Invalid terminal session"
        );
        let directory = &self.directory;
        let directory = if let Some(relative) = directory.strip_prefix("~/") {
            format!("\"$HOME\"/{}", transport::quote(relative))
        } else {
            transport::quote(directory)
        };
        let connect = format!(
            "ssh -tt -o ConnectTimeout=10 -o ServerAliveInterval=10 -o ServerAliveCountMax=2 -- {} {}",
            transport::quote(&self.host),
            transport::quote(&format!(
                "exec tmux -L xenon new-session -A -s {} -c {}",
                transport::quote(session),
                directory
            ))
        );
        Ok(format!(
            "while :; do {connect}; code=$?; if [ \"$code\" -ne 255 ]; then exit \"$code\"; fi; printf '\\r\\nSSH disconnected. Reconnecting…\\r\\n'; sleep 2; done"
        ))
    }

    fn request(&self, mut request: serde_json::Value) -> Result<serde_json::Value> {
        request["root"] = json!(self.directory);
        transport::request(&self.host, &request)
    }
}

#[cfg(test)]
mod tests;
