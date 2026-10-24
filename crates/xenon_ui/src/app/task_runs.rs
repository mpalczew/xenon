//! Task runs: the model behind Run Task. Each run owns a dependsOn
//! `Orchestrator` (xenon_core) and the task terminals it launched; terminal
//! events advance it. Views only draw; the phone remote drives the same
//! `start_task` as the picker.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use super::*;
use crate::task_input::{TaskInputView, TaskQuestion};
use xenon_core::{
    Action, EditorInfo, GroupKind, Orchestrator, Readiness, Reveal, TaskSet, VarContext,
    resolve_task,
};

pub(crate) type StartedFn = Rc<dyn Fn(&str, TabId)>;

/// Where and how a task terminal is placed.
pub(super) struct Placement {
    pub workspace: WorkspaceId,
    /// Always a new, revealed, focused tab (the picker's ⌘Return).
    pub force_new: bool,
    /// Never move the Mac's tab or focus (phone and folderOpen runs).
    pub quiet: bool,
}

/// A run started from a paired phone. `ask` sends it one input question (the
/// answer returns through `answer_task_input` with the same token and `conn`);
/// `started` reports the root task's tab once its terminal exists.
#[derive(Clone)]
pub(crate) struct RemoteOrigin {
    pub conn: u64,
    pub ask: Rc<dyn Fn(u64, TaskQuestion)>,
    pub started: StartedFn,
}

#[derive(Clone)]
pub(crate) enum Origin {
    /// The Mac: reveal and focus follow the task's presentation.
    Mac,
    /// Started by the app itself (folderOpen): never moves the Mac's tab or
    /// focus, and asks its inputs on the Mac.
    Auto,
    /// A paired phone: never moves the Mac's tab or focus.
    Remote(RemoteOrigin),
}

pub(crate) struct RunRequest {
    pub workspace: WorkspaceId,
    pub label: String,
    /// Run in a new tab of its own, revealed (the picker's ⌘Return).
    pub new_tab: bool,
    pub origin: Origin,
}

/// A tasks.json found for a workspace.
pub(crate) struct LoadedTasks {
    pub set: Arc<TaskSet>,
    /// Directory that holds `.vscode/` (`${workspaceFolder}`).
    pub folder: PathBuf,
}

/// Everything a run needs once its inputs are known.
pub(super) struct RunPlan {
    pub workspace: WorkspaceId,
    pub loaded: Arc<TaskSet>,
    pub folder: PathBuf,
    pub orch: Orchestrator,
    pub root_label: String,
    pub new_tab: bool,
    pub origin: Origin,
    pub editor: Option<EditorInfo>,
    pub config: BTreeMap<String, String>,
    pub inputs: BTreeMap<String, String>,
    pub secrets: BTreeSet<String>,
}

pub(super) struct PendingInputs {
    pub token: u64,
    pub plan: RunPlan,
    /// Input ids still to ask, in order.
    pub ids: Vec<String>,
}

struct Launched {
    view: Entity<TerminalView>,
    /// Background task: readiness tracker and how many lines it has seen.
    watch: Option<(Readiness, usize)>,
    ready: bool,
}

struct ActiveRun {
    id: u64,
    plan: RunPlan,
    launched: BTreeMap<String, Launched>,
}

#[derive(Default)]
pub(super) struct TaskRuns {
    runs: Vec<ActiveRun>,
    next: u64,
    pub pending: Option<PendingInputs>,
    pub input: Option<(Entity<TaskInputView>, Subscription)>,
    pub trust: Option<super::task_trust::TrustPrompt>,
    pub(super) shared_tabs: HashMap<WorkspaceId, Entity<TerminalView>>,
    pub(super) dedicated: HashMap<(WorkspaceId, String), Entity<TerminalView>>,
    pub folder_checked: HashSet<WorkspaceId>,
}

impl TaskRuns {
    pub fn next_token(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

impl XenonApp {
    /// Load `.vscode/tasks.json` for a workspace.
    pub(crate) fn load_tasks(&self, workspace: WorkspaceId) -> Result<LoadedTasks, String> {
        let root = self
            .workspace_root(workspace)
            .ok_or("that workspace is gone")?;
        if self.ssh_for_root(&root).is_some() {
            return Err("tasks run on this Mac; SSH workspaces are not supported".into());
        }
        let path = xenon_core::find_tasks_json(&root)
            .ok_or_else(|| format!("no .vscode/tasks.json above {}", root.display()))?;
        let folder = xenon_core::workspace_for_tasks_json(&path)
            .ok_or_else(|| format!("invalid tasks path {}", path.display()))?;
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let set = xenon_core::parse_task_set(&text, xenon_core::Os::current())?;
        Ok(LoadedTasks {
            set: Arc::new(set),
            folder,
        })
    }

    /// Plan a run and ask for its inputs, or launch straight away.
    pub(crate) fn start_task(
        &mut self,
        req: RunRequest,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let loaded = self.load_tasks(req.workspace)?;
        let orch = Orchestrator::new(&loaded.set, &req.label)?;
        let labels = orch.labels();
        let ids = xenon_core::required_inputs(&loaded.set, &labels);
        for id in &ids {
            match loaded.set.inputs.get(id).map(|i| &i.kind) {
                None => return Err(format!("${{input:{id}}} has no entry in \"inputs\"")),
                Some(xenon_core::InputKind::Command) => {
                    return Err(format!(
                        "input {id:?} uses a VS Code command, which Xenon cannot run"
                    ));
                }
                Some(_) => {}
            }
        }
        let secrets = ids
            .iter()
            .filter(|id| loaded.set.inputs.get(*id).is_some_and(|i| i.is_secret()))
            .cloned()
            .collect();
        let plan = RunPlan {
            workspace: req.workspace,
            loaded: loaded.set,
            folder: loaded.folder,
            orch,
            root_label: req.label,
            new_tab: req.new_tab,
            origin: req.origin,
            // File variables read the Mac's active editor pane, which only
            // belongs to the workspace on screen.
            editor: (self.active == Some(req.workspace))
                .then(|| self.editor_info(cx))
                .flatten(),
            config: settings_config(cx),
            inputs: BTreeMap::new(),
            secrets,
        };
        self.services.tasks.input = None;
        let token = self.services.tasks.next_token();
        self.services.tasks.pending = Some(PendingInputs { token, plan, ids });
        self.ask_next_input(cx);
        Ok(())
    }

    /// Active editor pane's file, cursor line, and selection.
    fn editor_info(&self, cx: &App) -> Option<EditorInfo> {
        let view = self.active_editor()?;
        let view = view.read(cx);
        let (row, _) = view.cursor_position().unwrap_or((0, 0));
        Some(EditorInfo {
            file: view.path().to_path_buf(),
            line: row + 1,
            selected_text: view.selected_text(),
        })
    }

    /// Inputs are collected; plan the terminals.
    pub(super) fn launch_plan(&mut self, plan: RunPlan, cx: &mut Context<Self>) {
        let id = self.services.tasks.next_token();
        let mut run = ActiveRun {
            id,
            plan,
            launched: BTreeMap::new(),
        };
        let actions = run.plan.orch.start();
        self.services.tasks.runs.push(run);
        self.apply_actions(id, actions, cx);
    }

    fn apply_actions(&mut self, run_id: u64, actions: Vec<Action>, cx: &mut Context<Self>) {
        let mut queue: std::collections::VecDeque<Action> = actions.into();
        while let Some(Action::Launch(label)) = queue.pop_front() {
            let composite = self
                .run_ref(run_id)
                .and_then(|r| r.plan.loaded.get(&label))
                .is_some_and(|d| d.command.is_none());
            if composite {
                // Only dependsOn: nothing to run once its dependencies are done.
                if let Some(run) = self.run_mut(run_id) {
                    queue.extend(run.plan.orch.satisfied(&label));
                }
            } else if let Err(error) = self.launch_one(run_id, &label, cx) {
                self.fail_run(run_id, &label, &error, cx);
                return;
            }
        }
        self.finish_if_done(run_id);
    }

    fn launch_one(
        &mut self,
        run_id: u64,
        label: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let run = self.run_ref(run_id).ok_or("run is gone")?;
        let plan = &run.plan;
        let def = plan.loaded.get(label).ok_or("task is gone")?;
        let exe = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let env = |k: &str| std::env::var(k).ok();
        let build_default = plan
            .loaded
            .default_of(GroupKind::Build)
            .map(|t| t.label.as_str());
        let ctx = VarContext {
            workspace: &plan.folder,
            editor: plan.editor.as_ref(),
            env: &env,
            config: &plan.config,
            inputs: &plan.inputs,
            secret_inputs: &plan.secrets,
            default_build_task: build_default,
            exec_path: &exe,
            user_home: &home,
        };
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        let resolved = resolve_task(def, &ctx, &shell)?;
        let (workspace, folder) = (plan.workspace, plan.folder.clone());
        let (root_label, origin) = (plan.root_label.clone(), plan.origin.clone());
        let force_new = plan.new_tab && label == plan.root_label;
        let quiet = !matches!(plan.origin, Origin::Mac);
        let watch = resolved
            .is_background
            .then(|| (Readiness::new(resolved.background.as_ref()), 0));
        let view = self.spawn_task_terminal(&folder, &resolved, cx);
        let placement = Placement {
            workspace,
            force_new,
            quiet,
        };
        self.place_task_terminal(&resolved, view.clone(), placement, cx);
        if label == root_label
            && let Origin::Remote(remote) = &origin
            && let Some((_, tab)) = self.locate_terminal(&view)
        {
            (remote.started)(label, tab);
        }
        if let Some(run) = self.run_mut(run_id) {
            run.launched.insert(
                label.to_string(),
                Launched {
                    view,
                    watch,
                    ready: false,
                },
            );
        }
        Ok(())
    }

    fn run_ref(&self, id: u64) -> Option<&ActiveRun> {
        self.services.tasks.runs.iter().find(|r| r.id == id)
    }

    fn run_mut(&mut self, id: u64) -> Option<&mut ActiveRun> {
        self.services.tasks.runs.iter_mut().find(|r| r.id == id)
    }

    /// The run and label that own a terminal.
    fn run_of(&self, view: &Entity<TerminalView>) -> Option<(u64, String)> {
        self.services.tasks.runs.iter().find_map(|run| {
            run.launched
                .iter()
                .find(|(_, l)| &l.view == view)
                .map(|(label, _)| (run.id, label.clone()))
        })
    }

    /// A task process exited.
    pub(super) fn on_task_finished(
        &mut self,
        view: Entity<TerminalView>,
        code: Option<i32>,
        cx: &mut Context<Self>,
    ) {
        let Some((run_id, label)) = self.run_of(&view) else {
            return;
        };
        let (pres, was_ready) = {
            let Some(run) = self.run_ref(run_id) else {
                return;
            };
            let pres = run
                .plan
                .loaded
                .get(&label)
                .map(|d| d.presentation)
                .unwrap_or_default();
            (pres, run.launched.get(&label).is_some_and(|l| l.ready))
        };
        if code == Some(0) {
            if pres.close {
                view.update(cx, |v, cx| {
                    v.set_auto_close(xenon_store::TerminalAutoClose::Immediate, cx)
                });
            }
            let actions = self
                .run_mut(run_id)
                .map(|r| r.plan.orch.satisfied(&label))
                .unwrap_or_default();
            self.apply_actions(run_id, actions, cx);
            return;
        }
        if pres.reveal == Reveal::Silent {
            self.reveal_terminal(&view);
        }
        if was_ready {
            return;
        }
        let reason = match code {
            Some(code) => format!("exited with code {code}"),
            None => "was terminated".to_string(),
        };
        self.fail_run(run_id, &label, &reason, cx);
    }

    /// Output arrived on a background task: has it become ready?
    pub(super) fn on_task_output(&mut self, view: Entity<TerminalView>, cx: &mut Context<Self>) {
        let Some((run_id, label)) = self.run_of(&view) else {
            return;
        };
        let lines = view.read(cx).all_text(cx);
        let became_ready = {
            let Some(l) = self
                .run_mut(run_id)
                .and_then(|r| r.launched.get_mut(&label))
            else {
                return;
            };
            let Some((readiness, seen)) = l.watch.as_mut() else {
                return;
            };
            let fresh: Vec<&str> = lines.iter().skip(*seen).map(String::as_str).collect();
            // The last line may still be growing: look at it again next time.
            *seen = lines.len().saturating_sub(1);
            let ready = readiness.feed(&fresh);
            l.ready |= ready;
            ready
        };
        if became_ready {
            let actions = self
                .run_mut(run_id)
                .map(|r| r.plan.orch.satisfied(&label))
                .unwrap_or_default();
            self.apply_actions(run_id, actions, cx);
        }
    }

    fn fail_run(&mut self, run_id: u64, label: &str, reason: &str, cx: &mut Context<Self>) {
        let root_label = self.run_ref(run_id).map(|r| r.plan.root_label.clone());
        if let Some(run) = self.run_mut(run_id) {
            run.plan.orch.failed(label, reason);
        }
        let detail = if root_label.as_deref() == Some(label) {
            reason.to_string()
        } else {
            format!("{reason}; the dependsOn chain stopped")
        };
        self.show_toast(
            super::toasts::failed(format!("Task {label} failed"), detail),
            cx,
        );
        self.finish_if_done(run_id);
    }

    fn finish_if_done(&mut self, run_id: u64) {
        let done = self
            .run_ref(run_id)
            .is_some_and(|r| r.plan.orch.outcome().is_some());
        if done {
            self.services.tasks.runs.retain(|r| r.id != run_id);
        }
    }
}

/// Xenon settings as `${config:key}` values: scalar leaves, dotted keys.
fn settings_config(cx: &App) -> BTreeMap<String, String> {
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
    if let Ok(value) = serde_json::to_value(xenon_settings::snapshot(cx)) {
        walk("", &value, &mut out);
    }
    out
}
