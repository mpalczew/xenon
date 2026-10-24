//! `${...}` variable expansion (VS Code variables reference).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// State of the active editor pane, for the file-derived variables.
#[derive(Debug, Clone, Default)]
pub struct EditorInfo {
    pub file: PathBuf,
    /// 1-based line of the cursor.
    pub line: u32,
    pub selected_text: String,
}

/// Everything variables can read. The caller gathers it; nothing here does I/O.
pub struct VarContext<'a> {
    pub workspace: &'a Path,
    pub editor: Option<&'a EditorInfo>,
    pub env: &'a dyn Fn(&str) -> Option<String>,
    /// Flattened Xenon settings for `${config:...}`.
    pub config: &'a BTreeMap<String, String>,
    pub inputs: &'a BTreeMap<String, String>,
    /// Input ids whose values must never be echoed.
    pub secret_inputs: &'a BTreeSet<String>,
    pub default_build_task: Option<&'a str>,
    pub exec_path: &'a str,
    pub user_home: &'a Path,
}

impl VarContext<'_> {
    /// Values to blank out of anything shown to the user.
    pub fn secret_values(&self) -> Vec<&str> {
        self.secret_inputs
            .iter()
            .filter_map(|id| self.inputs.get(id))
            .map(String::as_str)
            .filter(|v| !v.is_empty())
            .collect()
    }
}

/// Expand every `${...}` in `text`. Unknown variables are errors.
pub fn expand(text: &str, ctx: &VarContext) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            return Err(format!("unterminated variable in {text:?}"));
        };
        out.push_str(&lookup(&after[..end], ctx)?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Ids of every `${input:id}` in `text`.
pub fn input_refs(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("${input:") {
        let after = &rest[start + 8..];
        let Some(end) = after.find('}') else { break };
        ids.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    ids
}

fn lookup(name: &str, ctx: &VarContext) -> Result<String, String> {
    if let Some((kind, arg)) = name.split_once(':') {
        return match kind {
            "env" => Ok((ctx.env)(arg).unwrap_or_default()),
            "config" => ctx
                .config
                .get(arg)
                .cloned()
                .ok_or_else(|| format!("${{config:{arg}}}: no such Xenon setting")),
            "input" => ctx
                .inputs
                .get(arg)
                .cloned()
                .ok_or_else(|| format!("${{input:{arg}}}: no value was provided")),
            "workspaceFolder" | "workspaceRoot" if base(ctx.workspace) == arg => {
                path_text(ctx.workspace)
            }
            "workspaceFolder" | "workspaceRoot" => {
                Err(format!("${{{name}}}: no workspace folder named {arg:?}"))
            }
            _ => Err(format!("unknown variable ${{{name}}}")),
        };
    }
    match name {
        "workspaceFolder" | "workspaceRoot" | "cwd" | "fileWorkspaceFolder" => {
            path_text(ctx.workspace)
        }
        "workspaceFolderBasename" | "workspaceRootFolderName" => Ok(base(ctx.workspace)),
        "userHome" => path_text(ctx.user_home),
        "execPath" => Ok(ctx.exec_path.to_string()),
        "pathSeparator" | "/" => Ok(std::path::MAIN_SEPARATOR.to_string()),
        "defaultBuildTask" => Ok(ctx.default_build_task.unwrap_or_default().to_string()),
        _ => editor_var(name, ctx),
    }
}

fn editor_var(name: &str, ctx: &VarContext) -> Result<String, String> {
    const KNOWN: [&str; 9] = [
        "file",
        "relativeFile",
        "relativeFileDirname",
        "fileBasename",
        "fileBasenameNoExtension",
        "fileExtname",
        "fileDirname",
        "fileDirnameBasename",
        "lineNumber",
    ];
    if !KNOWN.contains(&name) && name != "selectedText" {
        return Err(format!("unknown variable ${{{name}}}"));
    }
    let Some(editor) = ctx.editor else {
        return Err(format!(
            "${{{name}}} needs a file open in the active editor pane"
        ));
    };
    let file = editor.file.as_path();
    let dir = file.parent().unwrap_or(Path::new(""));
    let rel = file.strip_prefix(ctx.workspace).unwrap_or(file);
    let rel_dir = rel.parent().unwrap_or(Path::new(""));
    match name {
        "file" => path_text(file),
        "relativeFile" => path_text(rel),
        "relativeFileDirname" => Ok(match path_text(rel_dir)?.as_str() {
            "" => ".".into(),
            other => other.to_string(),
        }),
        "fileBasename" => Ok(os_text(file.file_name())),
        "fileBasenameNoExtension" => Ok(os_text(file.file_stem())),
        "fileExtname" => Ok(file
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default()),
        "fileDirname" => path_text(dir),
        "fileDirnameBasename" => Ok(base(dir)),
        "lineNumber" => Ok(editor.line.to_string()),
        _ => Ok(editor.selected_text.clone()),
    }
}

fn base(p: &Path) -> String {
    os_text(p.file_name())
}

fn os_text(s: Option<&std::ffi::OsStr>) -> String {
    s.map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn path_text(p: &Path) -> Result<String, String> {
    Ok(p.to_string_lossy().into_owned())
}
