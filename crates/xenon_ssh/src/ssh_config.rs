//! Concrete host aliases from `~/.ssh/config`, following `Include`.

use std::path::{Path, PathBuf};

const MAX_INCLUDE_DEPTH: usize = 8;

/// Hosts named in the user's OpenSSH config. Wildcard and negated patterns are
/// not hosts you can connect to by name, so they are skipped.
pub fn config_hosts() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let ssh_dir = home.join(".ssh");
    let Ok(text) = std::fs::read_to_string(ssh_dir.join("config")) else {
        return Vec::new();
    };
    let mut read = |pattern: &str| read_matching(&home, &ssh_dir, pattern);
    collect_hosts(&text, &mut read, MAX_INCLUDE_DEPTH)
}

/// `read` returns the contents of every file an `Include` pattern matches.
pub(crate) fn collect_hosts(
    text: &str,
    read: &mut dyn FnMut(&str) -> Vec<String>,
    depth: usize,
) -> Vec<String> {
    let mut hosts = Vec::new();
    for (keyword, args) in directives(text) {
        match keyword.as_str() {
            "host" => hosts.extend(args.into_iter().filter(|a| is_concrete(a))),
            "include" if depth > 0 => {
                for pattern in &args {
                    for included in read(pattern) {
                        hosts.extend(collect_hosts(&included, read, depth - 1));
                    }
                }
            }
            _ => {}
        }
    }
    let mut unique = Vec::new();
    for host in hosts {
        if crate::valid_host(&host) && !unique.contains(&host) {
            unique.push(host);
        }
    }
    unique
}

fn is_concrete(pattern: &str) -> bool {
    !pattern.is_empty() && !pattern.contains(['*', '?', '!'])
}

/// `(lowercase keyword, arguments)` per non-comment line; `Key=value` allowed.
fn directives(text: &str) -> Vec<(String, Vec<String>)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let split = line.find(|c: char| c.is_whitespace() || c == '=')?;
            let keyword = line[..split].to_ascii_lowercase();
            let args = line[split..]
                .trim_start_matches(|c: char| c.is_whitespace() || c == '=')
                .split_whitespace()
                .map(|arg| arg.trim_matches('"').to_string())
                .collect();
            Some((keyword, args))
        })
        .collect()
}

fn read_matching(home: &Path, ssh_dir: &Path, pattern: &str) -> Vec<String> {
    let path = if let Some(rest) = pattern.strip_prefix("~/") {
        home.join(rest)
    } else if pattern.starts_with('/') {
        PathBuf::from(pattern)
    } else {
        ssh_dir.join(pattern)
    };
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| wildcard_match(&name, &entry.file_name().to_string_lossy()))
        .map(|entry| entry.path())
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok())
        .collect()
}

/// `*` and `?` in a single path component (the common `config.d/*` form).
pub(crate) fn wildcard_match(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut pi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(text: &str, files: &[(&str, &str)]) -> Vec<String> {
        let mut read = |pattern: &str| -> Vec<String> {
            files
                .iter()
                .filter(|(name, _)| wildcard_match(pattern, name))
                .map(|(_, body)| body.to_string())
                .collect()
        };
        collect_hosts(text, &mut read, MAX_INCLUDE_DEPTH)
    }

    #[test]
    fn concrete_hosts_only() {
        let text = "Host *\n  User me\nHost thinkpad thinkpad.local\n  HostName 10.0.0.2\n\
                    host=nuc\nHost *.corp !bad dev?\n# Host commented\nMatch host x\nHost one";
        assert_eq!(
            hosts(text, &[]),
            ["thinkpad", "thinkpad.local", "nuc", "one"]
        );
    }

    #[test]
    fn includes_are_followed_and_deduplicated() {
        let text = "Include conf.d/*\nHost alpha\nHost beta";
        let files = [
            ("conf.d/a", "Host beta\nHost gamma"),
            ("conf.d/b", "Include conf.d/c\nHost delta"),
            ("conf.d/c", "Host epsilon"),
            ("other", "Host nope"),
        ];
        assert_eq!(
            hosts(text, &files),
            ["beta", "gamma", "epsilon", "delta", "alpha"]
        );
    }

    #[test]
    fn include_cycles_terminate() {
        let files = [("loop", "Include loop\nHost looped")];
        assert_eq!(hosts("Include loop", &files), ["looped"]);
    }

    #[test]
    fn unsafe_aliases_are_dropped() {
        assert_eq!(hosts("Host -oProxyCommand=x ok;rm good", &[]), ["good"]);
    }

    #[test]
    fn wildcards_match_one_component() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("conf*.d", "conf-x.d"));
        assert!(wildcard_match("a?c", "abc"));
        assert!(!wildcard_match("a?c", "ac"));
        assert!(!wildcard_match("*.conf", "x.conf.bak"));
    }
}
