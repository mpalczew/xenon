use std::path::PathBuf;

use crate::ipc::{IpcRequest, OpenFileSpec, OpenPane};

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
    let mut workspace = std::env::var("XENON_WORKSPACE")
        .ok()
        .filter(|v| !v.is_empty());
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
            "--workspace" => {
                i += 1;
                if let Some(value) = args.get(i) {
                    workspace = Some(workspace_arg(value));
                }
            }
            flag if flag.starts_with("--workspace=") => {
                workspace = Some(workspace_arg(&flag[12..]));
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
        workspace,
    })
}

/// A workspace given as a relative path becomes absolute; a name or id stays as typed.
fn workspace_arg(value: &str) -> String {
    if value.contains('/') || value.starts_with('.') {
        abs_path(value.to_string()).to_string_lossy().into_owned()
    } else {
        value.to_string()
    }
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
