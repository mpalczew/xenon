//! `xenon open` argv: path, optional location, pane.

use std::path::PathBuf;

use crate::ipc::{IpcRequest, OpenFileSpec, OpenPane};

/// What a second process should send (or a cold start should apply).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Paths(Vec<PathBuf>),
    Open(OpenFileSpec),
    Activate,
}

impl CliCommand {
    pub fn to_request(&self) -> IpcRequest {
        match self {
            Self::Paths(paths) if paths.is_empty() => IpcRequest::Activate,
            Self::Paths(paths) => IpcRequest::Open {
                paths: paths.clone(),
            },
            Self::Open(spec) => IpcRequest::OpenFile(spec.clone()),
            Self::Activate => IpcRequest::Activate,
        }
    }
}

/// Parse argv after the binary name.
pub fn parse_cli(args: impl IntoIterator<Item = String>) -> CliCommand {
    let args: Vec<String> = args.into_iter().collect();
    if args.first().is_some_and(|a| a == "open") {
        return parse_open(&args[1..]);
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        return CliCommand::Activate;
    }
    let paths = crate::parse_cli_paths(args);
    if paths.is_empty() {
        CliCommand::Activate
    } else {
        CliCommand::Paths(paths)
    }
}

fn parse_open(args: &[String]) -> CliCommand {
    let mut pane = OpenPane::Sibling;
    let mut raw: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--" => {
                i += 1;
                if raw.is_none() && i < args.len() {
                    raw = Some(args[i].clone());
                }
                break;
            }
            "--pane" => {
                i += 1;
                if let Some(value) = args.get(i) {
                    pane = OpenPane::parse(value).unwrap_or(OpenPane::Sibling);
                }
            }
            flag if flag.starts_with("--pane=") => {
                pane = OpenPane::parse(&flag[7..]).unwrap_or(OpenPane::Sibling);
            }
            flag if flag.starts_with('-') => {}
            other if raw.is_none() => raw = Some(other.to_string()),
            _ => {}
        }
        i += 1;
    }
    let Some(raw) = raw else {
        return CliCommand::Activate;
    };
    let (path, line, column, end_line) = split_location(&raw);
    let path = abs_path(path);
    CliCommand::Open(OpenFileSpec {
        path,
        line,
        column,
        end_line,
        pane,
    })
}

fn abs_path(raw: String) -> PathBuf {
    let path = PathBuf::from(&raw);
    let abs = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    abs.canonicalize().unwrap_or(abs)
}

fn split_location(raw: &str) -> (String, Option<u32>, Option<u32>, Option<u32>) {
    let Some((path, rest)) = raw.rsplit_once(':') else {
        return (raw.to_string(), None, None, None);
    };
    if let Some((a, b)) = rest.split_once('-')
        && let (Ok(start), Ok(end)) = (a.parse::<u32>(), b.parse::<u32>())
        && start > 0
        && end > 0
    {
        return (path.to_string(), Some(start), None, Some(end));
    }
    let Ok(n) = rest.parse::<u32>() else {
        return (raw.to_string(), None, None, None);
    };
    if n == 0 {
        return (raw.to_string(), None, None, None);
    }
    let (inner, line, col, end) = split_location(path);
    if col.is_some() || end.is_some() {
        return (raw.to_string(), None, None, None);
    }
    match line {
        Some(line) => (inner, Some(line), Some(n), None),
        None => (inner, Some(n), None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> CliCommand {
        parse_cli(parts.iter().map(|s| (*s).to_string()))
    }

    fn spec(cmd: CliCommand) -> OpenFileSpec {
        match cmd {
            CliCommand::Open(spec) => spec,
            other => panic!("expected open, got {other:?}"),
        }
    }

    #[test]
    fn open_line() {
        let s = spec(args(&["open", "/tmp/file.rs:42"]));
        assert_eq!(s.path, PathBuf::from("/tmp/file.rs"));
        assert_eq!(s.line, Some(42));
        assert_eq!(s.column, None);
        assert_eq!(s.end_line, None);
        assert_eq!(s.pane, OpenPane::Sibling);
    }

    #[test]
    fn open_line_col() {
        let s = spec(args(&["open", "/tmp/file.rs:42:8"]));
        assert_eq!(s.line, Some(42));
        assert_eq!(s.column, Some(8));
        assert_eq!(s.end_line, None);
    }

    #[test]
    fn open_range() {
        let s = spec(args(&["open", "/tmp/file.rs:42-80"]));
        assert_eq!(s.line, Some(42));
        assert_eq!(s.end_line, Some(80));
        assert_eq!(s.column, None);
    }

    #[test]
    fn open_each_pane() {
        for (flag, pane) in [
            ("sibling", OpenPane::Sibling),
            ("focused", OpenPane::Focused),
            ("split-right", OpenPane::SplitRight),
        ] {
            let s = spec(args(&["open", "--pane", flag, "/tmp/a.rs"]));
            assert_eq!(s.pane, pane, "{flag}");
        }
        let s = spec(args(&["open", "/tmp/a.rs", "--pane=focused"]));
        assert_eq!(s.pane, OpenPane::Focused);
    }

    #[test]
    fn open_to_request_roundtrip() {
        let cmd = args(&["open", "--pane", "split-right", "/tmp/x.rs:3:1"]);
        let req = cmd.to_request();
        let json = serde_json::to_string(&req).unwrap();
        let back: IpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
        match back {
            IpcRequest::OpenFile(spec) => {
                assert_eq!(spec.line, Some(3));
                assert_eq!(spec.column, Some(1));
                assert_eq!(spec.pane, OpenPane::SplitRight);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn legacy_paths_still_parse() {
        match args(&["/abs"]) {
            CliCommand::Paths(paths) => assert!(paths[0].is_absolute()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn colon_name_without_digits_is_path() {
        let s = spec(args(&["open", "/tmp/foo:bar"]));
        assert_eq!(s.path, PathBuf::from("/tmp/foo:bar"));
        assert_eq!(s.line, None);
    }
}
