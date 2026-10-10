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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_read_only_host_ops() {
        assert_eq!(
            HostQuery::Name("xen".into()).request(),
            json!({"op": "discover", "needle": "xen"})
        );
        let dirs = HostQuery::Dirs {
            dir: "~/src".into(),
            prefix: "x".into(),
        };
        assert_eq!(
            dirs.request(),
            json!({"op": "list_dirs", "path": "~/src", "prefix": "x"})
        );
    }

    #[test]
    fn listings_decode_and_bad_hosts_never_reach_ssh() {
        let value = json!({"home": "/h", "dirs": [{"path": "/h/a", "git": true, "quality": 0}]});
        let listing: RemoteListing = serde_json::from_value(value).unwrap();
        assert_eq!(
            listing.dirs[0],
            RemoteDir {
                path: "/h/a".into(),
                git: true
            }
        );
        assert!(lookup("-oProxyCommand=x", &HostQuery::Name("a".into())).is_err());
    }

    #[test]
    fn home_is_shortened_only_on_a_path_boundary() {
        assert_eq!(tilde_path("/home/me/src/x", "/home/me"), "~/src/x");
        assert_eq!(tilde_path("/home/me", "/home/me"), "~");
        assert_eq!(tilde_path("/home/meow/x", "/home/me"), "/home/meow/x");
        assert_eq!(tilde_path("/etc", "/home/me"), "/etc");
    }
}
