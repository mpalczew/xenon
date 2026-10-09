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

/// What a query typed inside a host means. Mirrors the local picker: a bare
/// name searches default roots, `<path> <name>` searches under that path, and
/// a path lists folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostQuery {
    Empty,
    Name(String),
    Scoped {
        root: String,
        needle: String,
    },
    /// Children of `dir` whose names start with `prefix`.
    Dirs {
        dir: String,
        prefix: String,
    },
}

impl HostQuery {
    pub fn parse(raw: &str) -> Self {
        let query = raw.trim();
        if query.is_empty() {
            return Self::Empty;
        }
        if let Some((first, rest)) = query.split_once(char::is_whitespace)
            && is_path_like(first)
            && !rest.trim().is_empty()
        {
            return Self::Scoped {
                root: first.to_string(),
                needle: rest.trim().to_string(),
            };
        }
        if !is_path_like(query) {
            return Self::Name(query.to_string());
        }
        let (dir, prefix) = match query.rsplit_once('/') {
            Some(("", prefix)) => ("/", prefix),
            Some((dir, prefix)) => (dir, prefix),
            None => (query, ""),
        };
        Self::Dirs {
            dir: dir.to_string(),
            prefix: prefix.to_string(),
        }
    }

    /// The name used to rank rows, as the local picker's ranking needle.
    pub fn needle(&self) -> Option<&str> {
        match self {
            Self::Name(needle) | Self::Scoped { needle, .. } => Some(needle),
            Self::Empty | Self::Dirs { .. } => None,
        }
    }

    pub(crate) fn request(&self) -> Option<Value> {
        match self {
            Self::Empty => None,
            Self::Name(needle) => Some(json!({"op": "discover", "needle": needle})),
            Self::Scoped { root, needle } => {
                Some(json!({"op": "discover", "needle": needle, "scope": root}))
            }
            Self::Dirs { dir, prefix } => {
                Some(json!({"op": "list_dirs", "path": dir, "prefix": prefix}))
            }
        }
    }
}

fn is_path_like(text: &str) -> bool {
    text.starts_with(['/', '~']) || text.contains('/')
}

/// `None` when the query asks for nothing.
pub fn lookup(host: &str, query: &HostQuery) -> Result<Option<RemoteListing>> {
    ensure!(
        crate::valid_host(host),
        "Use an SSH host alias or user@host"
    );
    let Some(request) = query.request() else {
        return Ok(None);
    };
    let value = crate::transport::request(host, &request)?;
    Ok(Some(serde_json::from_value(value)?))
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

    fn dirs(dir: &str, prefix: &str) -> HostQuery {
        HostQuery::Dirs {
            dir: dir.into(),
            prefix: prefix.into(),
        }
    }

    #[test]
    fn names_and_paths_are_told_apart() {
        assert_eq!(HostQuery::parse("  "), HostQuery::Empty);
        assert_eq!(HostQuery::parse("xen "), HostQuery::Name("xen".into()));
        assert_eq!(HostQuery::parse("~/src/"), dirs("~/src", ""));
        assert_eq!(HostQuery::parse("~/src/xe"), dirs("~/src", "xe"));
        assert_eq!(HostQuery::parse("~"), dirs("~", ""));
        assert_eq!(HostQuery::parse("/etc/"), dirs("/etc", ""));
        assert_eq!(HostQuery::parse("/e"), dirs("/", "e"));
        assert_eq!(HostQuery::parse("src/"), dirs("src", ""));
    }

    #[test]
    fn a_path_then_a_name_searches_under_that_path() {
        assert_eq!(
            HostQuery::parse("~/src xenon"),
            HostQuery::Scoped {
                root: "~/src".into(),
                needle: "xenon".into()
            }
        );
        assert_eq!(HostQuery::parse("my project").needle(), Some("my project"));
    }

    #[test]
    fn requests_are_read_only_host_ops() {
        assert_eq!(HostQuery::Empty.request(), None);
        assert_eq!(
            HostQuery::parse("xen").request().unwrap(),
            json!({"op": "discover", "needle": "xen"})
        );
        assert_eq!(
            HostQuery::parse("~/src/x").request().unwrap(),
            json!({"op": "list_dirs", "path": "~/src", "prefix": "x"})
        );
        assert_eq!(
            HostQuery::parse("/ src").request().unwrap(),
            json!({"op": "discover", "needle": "src", "scope": "/"})
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
        assert!(lookup("-oProxyCommand=x", &HostQuery::parse("a")).is_err());
        assert!(lookup("box", &HostQuery::Empty).unwrap().is_none());
    }

    #[test]
    fn home_is_shortened_only_on_a_path_boundary() {
        assert_eq!(tilde_path("/home/me/src/x", "/home/me"), "~/src/x");
        assert_eq!(tilde_path("/home/me", "/home/me"), "~");
        assert_eq!(tilde_path("/home/meow/x", "/home/me"), "/home/meow/x");
        assert_eq!(tilde_path("/etc", "/home/me"), "/etc");
    }
}
