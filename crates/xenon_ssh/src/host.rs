//! Read-only, host-level lookups that need no workspace: find folders by name
//! and list a folder's children. Blocking; run on a background executor.

use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RemoteDir {
    pub path: String,
    pub git: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RemoteListing {
    /// The remote home directory, for shortening paths to `~/…`.
    pub home: String,
    pub dirs: Vec<RemoteDir>,
}

/// A read-only lookup on a host: find folders by name, or list a folder's
/// children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostQuery {
    /// Fuzzy name search under the default roots.
    Name(String),
    /// Children of `dir` whose names start with `prefix`.
    Dirs { dir: String, prefix: String },
}

impl HostQuery {
    pub(crate) fn request(&self) -> Value {
        match self {
            Self::Name(needle) => json!({"op": "discover", "needle": needle}),
            Self::Dirs { dir, prefix } => {
                json!({"op": "list_dirs", "path": dir, "prefix": prefix})
            }
        }
    }
}

pub fn lookup(host: &str, query: &HostQuery) -> Result<RemoteListing> {
    ensure!(
        crate::valid_host(host),
        "Use an SSH host alias or user@host"
    );
    let value = crate::transport::request(host, &query.request())?;
    Ok(serde_json::from_value(value)?)
}

/// `/home/me/src/x` as `~/src/x` when under `home`.
pub fn tilde_path(path: &str, home: &str) -> String {
    match path.strip_prefix(home) {
        Some("") => "~".into(),
        Some(rest) if rest.starts_with('/') && !home.is_empty() => format!("~{rest}"),
        _ => path.into(),
    }
}
