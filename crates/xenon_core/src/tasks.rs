//! VS Code shell tasks (`.vscode/tasks.json`) — pure parse + resolve.
//! I/O (reading the file) stays at the call site.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A resolved shell task ready to inject into a terminal or run in a shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellTask {
    pub label: String,
    pub detail: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}

impl ShellTask {
    /// Full shell line: command + args, with args shell-quoted.
    pub fn command_line(&self) -> String {
        if self.args.is_empty() {
            return self.command.clone();
        }
        let mut line = self.command.clone();
        for arg in &self.args {
            line.push(' ');
            line.push_str(&shell_quote(arg));
        }
        line
    }

    /// Line suitable for PTY inject: optional exports, cd, then command + newline.
    pub fn inject_line(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.env {
            out.push_str("export ");
            out.push_str(&shell_quote(k));
            out.push('=');
            out.push_str(&shell_quote(v));
            out.push_str("; ");
        }
        out.push_str("cd ");
        out.push_str(&shell_quote(&self.cwd.to_string_lossy()));
        out.push_str(" && ");
        out.push_str(&self.command_line());
        out.push('\n');
        out
    }
}

/// Parse a tasks.json document. Non-shell tasks are skipped. `workspace` is
/// `${workspaceFolder}` / the directory that contains `.vscode/`.
pub fn parse_shell_tasks(json: &str, workspace: &Path) -> Result<Vec<ShellTask>, String> {
    let file: TaskFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for raw in file.tasks {
        if let Some(task) = resolve_task(raw, workspace)? {
            out.push(task);
        }
    }
    Ok(out)
}

/// Walk up from `start` looking for `.vscode/tasks.json`.
pub fn find_tasks_json(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".vscode").join("tasks.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Workspace root for a tasks.json path (`…/.vscode/tasks.json` → `…`).
pub fn workspace_for_tasks_json(tasks_json: &Path) -> Option<PathBuf> {
    tasks_json.parent()?.parent().map(|p| p.to_path_buf())
}

/// Load and resolve shell tasks for a working directory (workspace root).
pub fn load_shell_tasks(start: &Path) -> Result<Vec<ShellTask>, String> {
    let path = find_tasks_json(start)
        .ok_or_else(|| format!("no .vscode/tasks.json above {}", start.display()))?;
    let workspace = workspace_for_tasks_json(&path)
        .ok_or_else(|| format!("invalid tasks path {}", path.display()))?;
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    parse_shell_tasks(&text, &workspace)
}

#[derive(Debug, Deserialize)]
struct TaskFile {
    #[serde(default)]
    tasks: Vec<RawTask>,
}

#[derive(Debug, Deserialize)]
struct RawTask {
    label: Option<String>,
    #[serde(rename = "type")]
    task_type: Option<String>,
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    detail: Option<String>,
    options: Option<RawOptions>,
}

#[derive(Debug, Deserialize)]
struct RawOptions {
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

fn resolve_task(raw: RawTask, workspace: &Path) -> Result<Option<ShellTask>, String> {
    let task_type = raw.task_type.as_deref().unwrap_or("shell");
    if task_type != "shell" {
        return Ok(None);
    }
    let label = match raw.label {
        Some(l) if !l.is_empty() => l,
        _ => return Ok(None),
    };
    let command = match raw.command {
        Some(c) if !c.is_empty() => expand_vars(&c, workspace),
        _ => return Ok(None),
    };
    let args: Vec<String> = raw
        .args
        .into_iter()
        .map(|a| expand_vars(&a, workspace))
        .collect();
    let opts = raw.options.unwrap_or(RawOptions {
        cwd: None,
        env: BTreeMap::new(),
    });
    let cwd = match opts.cwd {
        Some(c) => PathBuf::from(expand_vars(&c, workspace)),
        None => workspace.to_path_buf(),
    };
    let env = opts
        .env
        .into_iter()
        .map(|(k, v)| (k, expand_vars(&v, workspace)))
        .collect();
    Ok(Some(ShellTask {
        label,
        detail: raw.detail,
        command,
        args,
        cwd,
        env,
    }))
}

fn expand_vars(s: &str, workspace: &Path) -> String {
    let root = workspace.to_string_lossy();
    s.replace("${workspaceFolder}", &root)
        .replace("${workspaceRoot}", &root)
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/' | b':' | b'='))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_shell_tasks_and_skips_others() {
        let json = r#"{
          "version": "2.0.0",
          "tasks": [
            { "label": "build", "type": "shell", "command": "cargo build", "detail": "release" },
            { "label": "proc", "type": "process", "command": "echo" },
            { "label": "test", "command": "cargo test",
              "options": { "cwd": "${workspaceFolder}/ts", "env": { "A": "1" } } }
          ]
        }"#;
        let ws = Path::new("/repo");
        let tasks = parse_shell_tasks(json, ws).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].label, "build");
        assert_eq!(tasks[0].command, "cargo build");
        assert_eq!(tasks[1].cwd, PathBuf::from("/repo/ts"));
        assert_eq!(tasks[1].env.get("A").map(String::as_str), Some("1"));
    }

    #[test]
    fn inject_line_cds_and_exports() {
        let task = ShellTask {
            label: "t".into(),
            detail: None,
            command: "echo hi".into(),
            args: vec![],
            cwd: PathBuf::from("/tmp/ws"),
            env: BTreeMap::from([("FOO".into(), "bar baz".into())]),
        };
        let line = task.inject_line();
        assert!(line.contains("export FOO='bar baz'"));
        assert!(line.contains("cd /tmp/ws && echo hi\n") || line.contains("cd '/tmp/ws'"));
        assert!(line.ends_with('\n'));
    }
}
