//! VS Code `tasks.json` support: pure parse, resolve, and orchestration.
//! I/O (reading the file, spawning terminals) stays at the call site.

mod background;
mod model;
mod parse;
mod plan;
mod resolve;
mod trust;
mod vars;

use std::path::{Path, PathBuf};

pub use background::Readiness;
pub use model::{
    Background, Group, GroupKind, InputDef, InputKind, Panel, PickOption, Presentation, Quoted,
    Quoting, Reveal, ShellOptions, TaskDef, TaskKind, TaskOptions, TaskSet,
};
pub use parse::{Os, parse_task_set};
pub use plan::{Action, Orchestrator, Outcome};
pub use resolve::{ResolvedTask, preview_line, resolve_task};
pub use trust::{folder_open_closure, folder_open_hash};
pub use vars::{EditorInfo, VarContext, expand, input_refs};

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

/// Input ids a run of `root` needs: every `${input:id}` in the task and its
/// dependency closure, in first-seen order.
pub fn required_inputs(set: &TaskSet, labels: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for label in labels {
        let Some(def) = set.get(label) else { continue };
        for text in def.strings() {
            for id in input_refs(text) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests;
