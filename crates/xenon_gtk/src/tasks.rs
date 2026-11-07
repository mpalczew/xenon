//! Task runs: the model behind Run Task. Each run owns a dependsOn
//! `Orchestrator` (xenon_core) and the task terminals it launched; terminal
//! signals advance it. Rules come from `xenon_core::tasks`; the picker, the
//! input prompts and the folderOpen card are drawn in `tasks_ui.rs`.

mod folder;
mod place;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use xenon_core::tasks::{
    self as shared, BuildChoice, LoadedTasks, RunVars, TaskQuestion, choose_build_task,
    resolve_for_run, terminal_command,
};
use xenon_core::{
    Action, EditorInfo, Orchestrator, Readiness, TabId, TabState, TaskSet, WorkspaceId,
};

use crate::app::App;
use crate::model::TabKind;

/// How a task terminal starts, instead of the shell.
pub(crate) struct Spawn {
    pub cwd: PathBuf,
    pub argv: Vec<String>,
    pub env: Vec<String>,
}

/// Where a task terminal goes.
pub(crate) struct Placement {
    /// Always a new, revealed, focused tab (the picker's Ctrl+Return).
    force_new: bool,
    /// Never move the user's tab or focus (folderOpen runs).
    quiet: bool,
}

/// Everything a run needs once its inputs are known.
pub(crate) struct RunPlan {
    ws: WorkspaceId,
    set: Arc<TaskSet>,
    folder: PathBuf,
    orch: Orchestrator,
    root: String,
    /// Run the root task in a new tab of its own, revealed (the picker's Ctrl+Return).
    new_tab: bool,
    /// Started by the app itself (folderOpen): never moves the tab or focus.
    quiet: bool,
    vars: RunVars,
}

struct Pending {
    plan: RunPlan,
    /// Input ids still to ask, in order.
    ids: Vec<String>,
}

pub(crate) struct Launched {
    /// Background task: readiness tracker and how many lines it has seen.
    watch: Option<(Readiness, usize)>,
    ready: bool,
    /// The process ended.
    done: bool,
}

pub(crate) struct ActiveRun {
    id: u64,
    plan: RunPlan,
    launched: BTreeMap<String, Launched>,
}

/// What folderOpen approval is being asked.
pub(crate) struct TrustPrompt {
    ws: WorkspaceId,
    folder: PathBuf,
    hash: String,
}

#[derive(Default)]
pub(crate) struct Tasks {
    runs: RefCell<Vec<ActiveRun>>,
    next: Cell<u64>,
    pending: RefCell<Option<Pending>>,
    /// Task tabs: the run and task label that own them.
    owners: RefCell<HashMap<(WorkspaceId, TabId), (u64, String)>>,
    /// Terminal tabs that start a task instead of a shell.
    spawns: RefCell<HashMap<(WorkspaceId, TabId), Spawn>>,
    shared: RefCell<HashMap<WorkspaceId, TabId>>,
    dedicated: RefCell<HashMap<(WorkspaceId, String), TabId>>,
    folder_checked: RefCell<HashSet<WorkspaceId>>,
    trust: RefCell<Option<TrustPrompt>>,
}

impl Tasks {
    pub(crate) fn take_spawn(&self, ws: WorkspaceId, tab: TabId) -> Option<Spawn> {
        self.spawns.borrow_mut().remove(&(ws, tab))
    }

    pub(crate) fn owns(&self, ws: WorkspaceId, tab: TabId) -> bool {
        self.owners.borrow().contains_key(&(ws, tab))
    }
}

impl App {
    /// A failure as a toast: the title, then what went wrong.
    pub(crate) fn toast_failed(&self, title: &str, detail: &str) {
        self.toast(&format!("{title} \u{2014} {detail}"));
    }

    /// Load `.vscode/tasks.json` for a workspace.
    pub(crate) fn load_workspace_tasks(&self, ws: WorkspaceId) -> Result<LoadedTasks, String> {
        let model = self.model.borrow();
        let rec = model.rec(ws).ok_or("that workspace is gone")?;
        if rec.ssh.is_some() {
            return Err(shared::SSH_TASKS.into());
        }
        shared::load_tasks(&rec.root)
    }

    pub(crate) fn no_workspace_toast(&self) {
        let (title, detail) = shared::PICK_WORKSPACE;
        self.toast_failed(title, detail);
    }

    /// Ctrl+Shift+B: the default build task; else the only build task; else
    /// the picker.
    pub(crate) fn run_build_task(self: &Rc<Self>) {
        let Some(ws) = self.model.borrow().active() else {
            self.no_workspace_toast();
            return;
        };
        let loaded = match self.load_workspace_tasks(ws) {
            Ok(loaded) => loaded,
            Err(error) => {
                self.toast_failed(shared::LOAD_FAILED, &error);
                return;
            }
        };
        match choose_build_task(&loaded.set) {
            BuildChoice::Run(label) => self.run_task_checked(ws, label, false),
            BuildChoice::None => {
                let (title, detail) = shared::NO_BUILD_TASK;
                self.toast_failed(title, detail);
            }
            BuildChoice::Pick => self.open_task_picker(),
        }
    }

    /// Start a task from the user, toasting a failure to even start.
    pub(crate) fn run_task_checked(self: &Rc<Self>, ws: WorkspaceId, label: String, new_tab: bool) {
        if let Err(error) = self.start_task(ws, label, new_tab, false) {
            self.toast_failed(shared::COULDNT_RUN, &error);
        }
    }

    /// Plan a run and ask for its inputs, or launch straight away.
    pub(super) fn start_task(
        self: &Rc<Self>,
        ws: WorkspaceId,
        label: String,
        new_tab: bool,
        quiet: bool,
    ) -> Result<(), String> {
        let loaded = self.load_workspace_tasks(ws)?;
        let shared::RunSetup { orch, ids, secrets } = shared::plan_run(&loaded.set, &label)?;
        let plan = RunPlan {
            ws,
            set: loaded.set,
            folder: loaded.folder,
            orch,
            root: label,
            new_tab,
            quiet,
            vars: RunVars {
                editor: self.editor_info(ws),
                config: settings_config(),
                inputs: BTreeMap::new(),
                secrets,
            },
        };
        *self.tasks.pending.borrow_mut() = Some(Pending { plan, ids });
        self.ask_next_input();
        Ok(())
    }

    /// Active editor's file, cursor line, and selection.
    fn editor_info(&self, ws: WorkspaceId) -> Option<EditorInfo> {
        let tab = {
            let model = self.model.borrow();
            if model.active() != Some(ws) {
                return None;
            }
            let tab = model.spaces.get(&ws)?.content.active_tab()?;
            matches!(tab.kind, TabKind::Kept(TabState::Editor { .. })).then_some(tab.id)?
        };
        let editor = self.editors.borrow().get(&(ws, tab)).cloned()?;
        let buffer = &editor.buffer;
        let line = buffer.iter_at_offset(buffer.cursor_position()).line();
        let selected_text = buffer
            .selection_bounds()
            .map(|(a, b)| buffer.text(&a, &b, false).to_string())
            .unwrap_or_default();
        Some(EditorInfo {
            file: editor.path.clone(),
            line: line as u32 + 1,
            selected_text,
        })
    }

    /// Ask the first unanswered input, or launch when none remain.
    fn ask_next_input(self: &Rc<Self>) {
        let question = {
            let pending = self.tasks.pending.borrow();
            let Some(pending) = pending.as_ref() else {
                return;
            };
            pending
                .ids
                .first()
                .map(|id| pending.plan.set.inputs.get(id).and_then(TaskQuestion::of))
        };
        match question {
            None => {
                let pending = self.tasks.pending.borrow_mut().take();
                if let Some(pending) = pending {
                    self.launch_plan(pending.plan);
                }
            }
            Some(Some(question)) => self.show_input_prompt(question),
            Some(None) => {}
        }
    }

    /// An answer (`Some`) or a cancel (`None`, which ends the run) for the
    /// pending question.
    pub(crate) fn answer_task_input(self: &Rc<Self>, value: Option<String>) {
        {
            let mut pending = self.tasks.pending.borrow_mut();
            match (pending.as_mut(), value) {
                (Some(p), Some(value)) if !p.ids.is_empty() => {
                    let id = p.ids.remove(0);
                    p.plan.vars.inputs.insert(id, value);
                }
                _ => {
                    *pending = None;
                    return;
                }
            }
        }
        self.ask_next_input();
    }

    pub(super) fn with_run<R>(&self, id: u64, f: impl FnOnce(&mut ActiveRun) -> R) -> Option<R> {
        let mut runs = self.tasks.runs.borrow_mut();
        runs.iter_mut().find(|r| r.id == id).map(f)
    }

    /// Inputs are collected; plan the terminals.
    fn launch_plan(self: &Rc<Self>, plan: RunPlan) {
        let id = self.tasks.next.get() + 1;
        self.tasks.next.set(id);
        let mut run = ActiveRun {
            id,
            plan,
            launched: BTreeMap::new(),
        };
        let actions = run.plan.orch.start();
        self.tasks.runs.borrow_mut().push(run);
        self.apply_actions(id, actions);
    }

    pub(super) fn apply_actions(self: &Rc<Self>, run_id: u64, actions: Vec<Action>) {
        let mut queue: VecDeque<Action> = actions.into();
        while let Some(Action::Launch(label)) = queue.pop_front() {
            let composite = self
                .with_run(run_id, |r| {
                    r.plan.set.get(&label).is_some_and(|d| d.command.is_none())
                })
                .unwrap_or(false);
            if composite {
                queue.extend(
                    self.with_run(run_id, |r| r.plan.orch.satisfied(&label))
                        .unwrap_or_default(),
                );
            } else if let Err(error) = self.launch_one(run_id, &label) {
                self.fail_run(run_id, &label, &error);
                return;
            }
        }
        self.finish_if_done(run_id);
    }

    fn launch_one(self: &Rc<Self>, run_id: u64, label: &str) -> Result<(), String> {
        let (resolved, ws, force_new, quiet) = self
            .with_run(run_id, |run| {
                let plan = &run.plan;
                let def = plan.set.get(label).ok_or("task is gone")?;
                let resolved = resolve_for_run(&plan.set, &plan.folder, def, &plan.vars)?;
                let force_new = plan.new_tab && label == plan.root;
                Ok::<_, String>((resolved, plan.ws, force_new, plan.quiet))
            })
            .ok_or("run is gone")??;
        let (program, args) = terminal_command(&resolved);
        let spawn = Spawn {
            cwd: resolved.cwd.clone(),
            argv: std::iter::once(program).chain(args).collect(),
            env: resolved
                .env
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect(),
        };
        let tab = self
            .place_task_terminal(&resolved, ws, Placement { force_new, quiet }, spawn)
            .ok_or("that workspace is gone")?;
        self.tasks
            .owners
            .borrow_mut()
            .insert((ws, tab), (run_id, label.to_string()));
        let watch = resolved
            .is_background
            .then(|| (Readiness::new(resolved.background.as_ref()), 0));
        self.with_run(run_id, |run| {
            run.launched.insert(
                label.to_string(),
                Launched {
                    watch,
                    ready: false,
                    done: false,
                },
            );
        });
        Ok(())
    }

    pub(super) fn fail_run(self: &Rc<Self>, run_id: u64, label: &str, reason: &str) {
        let root = self.with_run(run_id, |run| {
            run.plan.orch.failed(label, reason);
            run.plan.root.clone()
        });
        let (title, detail) =
            shared::failure_toast(root.as_deref().unwrap_or_default(), label, reason);
        self.toast_failed(&title, &detail);
        self.finish_if_done(run_id);
    }

    fn finish_if_done(&self, run_id: u64) {
        let done = self
            .with_run(run_id, |r| r.plan.orch.outcome().is_some())
            .unwrap_or(false);
        if done {
            self.tasks.runs.borrow_mut().retain(|r| r.id != run_id);
        }
    }
}

/// Xenon settings as `${config:key}` values.
fn settings_config() -> BTreeMap<String, String> {
    xenon_store::load_settings()
        .ok()
        .and_then(|settings| serde_json::to_value(settings).ok())
        .map(|value| shared::flatten_config(&value))
        .unwrap_or_default()
}
