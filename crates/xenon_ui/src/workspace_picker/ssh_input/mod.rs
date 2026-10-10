//! The `ssh://` part of the Open Workspace query: what the text means, what to
//! ask the host, and what Tab writes back. Pure, so it is tested without a window.
//!
//! `ssh://host/<rest>`: `<rest>` is relative to the remote home unless it starts
//! with `/` (absolute) or `~` (home).

use xenon_ssh::{HostQuery, tilde_path};

const SCHEME: &str = "ssh://";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SshInput<'a> {
    /// `ssh://thin`: still choosing a host.
    Hosts(&'a str),
    /// `ssh://thinkpad/src/xe`
    Path { host: &'a str, rest: &'a str },
}

impl<'a> SshInput<'a> {
    pub(super) fn parse(query: &'a str) -> Option<Self> {
        let after = query.trim_start().strip_prefix(SCHEME)?;
        Some(match after.split_once('/') {
            Some((host, rest)) if !host.is_empty() => Self::Path { host, rest },
            _ => Self::Hosts(after.trim_start_matches('/')),
        })
    }
}

/// `ss` + Tab becomes `ssh://`.
pub(super) fn scheme_completion(query: &str) -> Option<&'static str> {
    (query.len() >= 2 && query.len() < SCHEME.len() && SCHEME.starts_with(query)).then_some(SCHEME)
}

pub(super) fn host_matches(host: &str, prefix: &str) -> bool {
    host.to_lowercase().starts_with(&prefix.to_lowercase())
}

pub(super) fn host_completion(host: &str) -> String {
    format!("{SCHEME}{host}/")
}

/// Parent folder and the partly typed last segment of `rest`.
fn split_rest(rest: &str) -> (&str, &str) {
    match rest.rsplit_once('/') {
        Some(("", prefix)) => ("/", prefix),
        Some((dir, prefix)) => (dir, prefix),
        None if rest.starts_with('~') => ("~", ""),
        None => ("~", rest),
    }
}

/// The partly typed last segment, which is what name search looks for.
pub(super) fn last_segment(rest: &str) -> &str {
    split_rest(rest).1
}

/// Folder listing first, then (when something is typed) a name search.
pub(super) fn lookups(rest: &str) -> (HostQuery, Option<HostQuery>) {
    let (dir, prefix) = split_rest(rest);
    let listing = HostQuery::Dirs {
        dir: dir.into(),
        prefix: prefix.into(),
    };
    let search = (!prefix.is_empty()).then(|| HostQuery::Name(prefix.into()));
    (listing, search)
}

/// The typed text already names a whole folder (`ssh://box/`, `~`, `src/`).
pub(super) fn names_whole_folder(rest: &str) -> bool {
    rest.is_empty() || rest == "~" || rest.ends_with('/')
}

/// Absolute remote path for a path in any of the three styles.
pub(super) fn absolute(path: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    match path {
        _ if path.starts_with('/') => path.to_string(),
        "~" | "" => home.to_string(),
        _ => format!("{home}/{}", path.strip_prefix("~/").unwrap_or(path)),
    }
}

/// Address the app opens for what the user typed.
pub(super) fn open_address(host: &str, rest: &str) -> String {
    let trimmed = rest.trim_end_matches('/');
    if rest.starts_with('/') {
        return format!(
            "{SCHEME}{host}{}",
            if trimmed.is_empty() { "/" } else { trimmed }
        );
    }
    match trimmed.strip_prefix("~/").unwrap_or(trimmed) {
        "" | "~" => format!("{SCHEME}{host}/~/"),
        path => format!("{SCHEME}{host}/~/{path}"),
    }
}

/// Address for a folder the host listed (absolute path).
pub(super) fn folder_address(host: &str, path: &str) -> String {
    format!("{SCHEME}{host}{path}")
}

/// Query text after Tab on the folder `path`: same style as what was typed,
/// ending in `/` so the next listing is that folder's children.
pub(super) fn path_completion(host: &str, rest: &str, path: &str, home: &str) -> String {
    let path = if home.is_empty() {
        path.to_string()
    } else {
        absolute(path, home)
    };
    let shown = if rest.starts_with('/') {
        path
    } else if rest.starts_with('~') {
        tilde_path(&path, home)
    } else {
        relative_to_home(&path, home)
    };
    let slash = if shown.ends_with('/') { "" } else { "/" };
    format!("{SCHEME}{host}/{shown}{slash}")
}

fn relative_to_home(path: &str, home: &str) -> String {
    let tilde = tilde_path(path, home);
    match tilde.strip_prefix("~/") {
        Some(relative) => relative.to_string(),
        None => tilde,
    }
}

#[cfg(test)]
mod tests;
