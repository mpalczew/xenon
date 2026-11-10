use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::model::{GroupKind, TaskDef, TaskSet};
use super::resolve::{ResolvedTask, resolve_task};
use super::vars::{EditorInfo, VarContext};

#[derive(Default)]
pub struct RunVars {
    pub editor: Option<EditorInfo>,
    pub config: BTreeMap<String, String>,
    pub inputs: BTreeMap<String, String>,
    pub secrets: BTreeSet<String>,
}

pub fn resolve_for_run(
    set: &TaskSet,
    folder: &Path,
    def: &TaskDef,
    vars: &RunVars,
) -> Result<ResolvedTask, String> {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let env = |k: &str| std::env::var(k).ok();
    let ctx = VarContext {
        workspace: folder,
        editor: vars.editor.as_ref(),
        env: &env,
        config: &vars.config,
        inputs: &vars.inputs,
        secret_inputs: &vars.secrets,
        default_build_task: set.default_of(GroupKind::Build).map(|t| t.label.as_str()),
        exec_path: &exe,
        user_home: &home,
    };
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
    resolve_task(def, &ctx, &shell)
}

pub fn terminal_command(task: &ResolvedTask) -> (String, Vec<String>) {
    if !task.presentation.echo {
        return (task.program.clone(), task.args.clone());
    }
    let mut args = vec![
        "-c".to_string(),
        "printf '\\033[2m> %s\\033[0m\\n\\n' \"$1\"; shift; exec \"$@\"".to_string(),
        "xenon-task".to_string(),
        task.display.clone(),
        task.program.clone(),
    ];
    args.extend(task.args.iter().cloned());
    ("/bin/sh".to_string(), args)
}

pub fn flatten_config(value: &serde_json::Value) -> BTreeMap<String, String> {
    fn walk(prefix: &str, value: &serde_json::Value, out: &mut BTreeMap<String, String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    walk(&key, v, out);
                }
            }
            serde_json::Value::String(s) => {
                out.insert(prefix.to_string(), s.clone());
            }
            serde_json::Value::Number(_) | serde_json::Value::Bool(_) => {
                out.insert(prefix.to_string(), value.to_string());
            }
            _ => {}
        }
    }
    let mut out = BTreeMap::new();
    walk("", value, &mut out);
    out
}

pub fn failure_toast(root: &str, label: &str, reason: &str) -> (String, String) {
    let detail = if root == label {
        reason.to_string()
    } else {
        format!("{reason}; the dependsOn chain stopped")
    };
    (format!("Task {label} failed"), detail)
}

pub fn exit_reason(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("exited with code {code}"),
        None => "was terminated".to_string(),
    }
}
