//! Finding a workspace's tasks.json, choosing the build task, and planning a run.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::model::{GroupKind, InputKind, TaskDef, TaskSet};
use super::plan::Orchestrator;
use super::{Os, find_tasks_json, parse_task_set, required_inputs, workspace_for_tasks_json};

/// A tasks.json found for a workspace.
pub struct LoadedTasks {
    pub set: Arc<TaskSet>,
    /// Directory that holds `.vscode/` (`${workspaceFolder}`).
    pub folder: PathBuf,
}

/// Load `.vscode/tasks.json` at or above a workspace root.
pub fn load_tasks(root: &Path) -> Result<LoadedTasks, String> {
    let path = find_tasks_json(root)
        .ok_or_else(|| format!("no .vscode/tasks.json above {}", root.display()))?;
    let folder = workspace_for_tasks_json(&path)
        .ok_or_else(|| format!("invalid tasks path {}", path.display()))?;
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let set = parse_task_set(&text, Os::current())?;
    Ok(LoadedTasks {
        set: Arc::new(set),
        folder,
    })
}

/// What Run Build Task does.
pub enum BuildChoice {
    /// The default build task, else the only build task.
    Run(String),
    /// Several build tasks, none the default: show the picker.
    Pick,
    /// No task is in the build group.
    None,
}

pub fn choose_build_task(set: &TaskSet) -> BuildChoice {
    let builds: Vec<&TaskDef> = set
        .tasks
        .iter()
        .filter(|t| t.group.is_some_and(|g| g.kind == GroupKind::Build))
        .collect();
    let chosen = set
        .default_of(GroupKind::Build)
        .or(if builds.len() == 1 {
            builds.first().copied()
        } else {
            None
        })
        .map(|t| t.label.clone());
    match chosen {
        Some(label) => BuildChoice::Run(label),
        None if builds.is_empty() => BuildChoice::None,
        None => BuildChoice::Pick,
    }
}

/// A run, planned: its dependsOn orchestrator, the inputs to ask in order, and
/// which of them are secret.
pub struct RunSetup {
    pub orch: Orchestrator,
    pub ids: Vec<String>,
    pub secrets: BTreeSet<String>,
}

pub fn plan_run(set: &TaskSet, label: &str) -> Result<RunSetup, String> {
    let orch = Orchestrator::new(set, label)?;
    let ids = required_inputs(set, &orch.labels());
    for id in &ids {
        match set.inputs.get(id).map(|i| &i.kind) {
            None => return Err(format!("${{input:{id}}} has no entry in \"inputs\"")),
            Some(InputKind::Command) => {
                return Err(format!(
                    "input {id:?} uses a VS Code command, which Xenon cannot run"
                ));
            }
            Some(_) => {}
        }
    }
    let secrets = ids
        .iter()
        .filter(|id| set.inputs.get(*id).is_some_and(|i| i.is_secret()))
        .cloned()
        .collect();
    Ok(RunSetup { orch, ids, secrets })
}
