//! Picker rows: what the Run Task list shows for each task.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::model::{GroupKind, TaskDef, TaskSet};
use super::resolve::preview_line;
use super::vars::{EditorInfo, VarContext};

/// One picker row.
#[derive(Clone, Debug)]
pub struct TaskRow {
    pub label: String,
    /// The command line as it will run (variables expanded where known).
    pub command_line: String,
    /// The task's own `detail` text.
    pub detail: Option<String>,
    /// Chip text, e.g. "build · default".
    pub tag: Option<String>,
    /// `build` or `test`.
    pub group: Option<&'static str>,
    pub is_default: bool,
    /// Why it cannot run here; such rows are listed dimmed.
    pub unsupported: Option<String>,
}

impl TaskRow {
    pub fn subtitle(&self) -> String {
        if let Some(reason) = &self.unsupported {
            return reason.clone();
        }
        match &self.detail {
            Some(detail) => format!("$ {}  ·  {detail}", self.command_line),
            None => format!("$ {}", self.command_line),
        }
    }

    /// Chip text: the tag, or "unsupported".
    pub fn chip(&self) -> Option<String> {
        self.unsupported
            .as_ref()
            .map(|_| "unsupported".to_string())
            .or_else(|| self.tag.clone())
    }
}

/// Empty-list text of the picker.
pub fn picker_empty_message(load_failed: bool, query_empty: bool) -> &'static str {
    if load_failed {
        "No tasks loaded"
    } else if query_empty {
        "No tasks in this workspace"
    } else {
        "No matching tasks"
    }
}

/// Run `f` with a variable context that knows only the workspace (no editor,
/// no inputs): what the picker and the trust prompt can show before a run.
pub(crate) fn with_preview_ctx<R>(
    set: &TaskSet,
    folder: &Path,
    f: impl FnOnce(&VarContext) -> R,
) -> R {
    let env = |k: &str| std::env::var(k).ok();
    let empty = BTreeMap::new();
    let secrets = BTreeSet::new();
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let editor: Option<&EditorInfo> = None;
    f(&VarContext {
        workspace: folder,
        editor,
        env: &env,
        config: &empty,
        inputs: &empty,
        secret_inputs: &secrets,
        default_build_task: set.default_of(GroupKind::Build).map(|t| t.label.as_str()),
        exec_path: "",
        user_home: &home,
    })
}

/// Picker rows: every non-hidden task, unsupported ones included, then one
/// row per skipped section.
pub fn task_rows(set: &TaskSet, folder: &Path) -> Vec<TaskRow> {
    let mut rows: Vec<TaskRow> = with_preview_ctx(set, folder, |ctx| {
        set.tasks
            .iter()
            .filter(|t| !t.hide)
            .map(|t| TaskRow {
                label: t.label.clone(),
                command_line: row_command(t, ctx),
                detail: t.detail.clone(),
                tag: group_tag(t),
                group: t.group.map(|g| group_name(g.kind)),
                is_default: t.group.is_some_and(|g| g.is_default),
                unsupported: t.unsupported.clone(),
            })
            .collect()
    });
    rows.extend(set.warnings.iter().map(|w| TaskRow {
        label: "Skipped".into(),
        command_line: String::new(),
        detail: None,
        tag: None,
        group: None,
        is_default: false,
        unsupported: Some(w.clone()),
    }));
    rows
}

/// The command line a row or trust line shows, with its dependencies.
pub(crate) fn row_command(task: &TaskDef, ctx: &VarContext) -> String {
    let line = preview_line(task, ctx);
    if task.depends_on.is_empty() {
        return line;
    }
    let deps = task.depends_on.join(", ");
    if line.is_empty() {
        format!("(runs {deps})")
    } else {
        format!("{line}  (after {deps})")
    }
}

fn group_name(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Build => "build",
        GroupKind::Test => "test",
    }
}

fn group_tag(task: &TaskDef) -> Option<String> {
    let group = task.group?;
    let kind = group_name(group.kind);
    Some(if group.is_default {
        format!("{kind} · default")
    } else {
        kind.to_string()
    })
}
