//! VS Code `tasks.json` support: pure parse, resolve, and orchestration.
//! I/O (reading the file, spawning terminals) stays at the call site.

mod background;
mod consent;
mod load;
mod model;
mod parse;
mod plan;
mod question;
mod resolve;
mod rows;
mod run;
mod texts;
mod trust;
mod vars;

use std::path::{Path, PathBuf};

pub use background::Readiness;
pub use consent::*;
pub use load::*;
pub use model::{Background, Panel, Reveal, TaskSet};

pub use plan::{Action, Orchestrator};
pub use question::*;
pub use resolve::ResolvedTask;
pub use rows::*;
pub use run::*;
pub use texts::*;
pub use trust::folder_open_hash;
pub use vars::EditorInfo;

/// Walk up from `start` looking for `.vscode/tasks.json`.
pub(crate) fn find_tasks_json(start: &Path) -> Option<PathBuf> {
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
pub(crate) fn workspace_for_tasks_json(tasks_json: &Path) -> Option<PathBuf> {
    tasks_json.parent()?.parent().map(|p| p.to_path_buf())
}

/// Input ids a run of `root` needs: every `${input:id}` in the task and its
/// dependency closure, in first-seen order.
pub(crate) fn required_inputs(set: &TaskSet, labels: &[String]) -> Vec<String> {
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

pub(crate) use parse::{Os, parse_task_set};
pub(crate) use vars::input_refs;
