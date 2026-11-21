use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use xenon_core::tasks::{
    self as shared, QuestionKind, RunVars, TaskQuestion, resolve_for_run, terminal_command,
};
use xenon_core::{Action, Orchestrator, TaskSet, TermId, WorkspaceId};
use xenon_remote::{ConnId, Secret, ServerMsg, TaskInputKind, TaskOptionWire};

use super::{Phone, locked};
use xenon_term::{Exit, Spawn};

struct Plan {
    ws: WorkspaceId,
    set: Arc<TaskSet>,
    folder: PathBuf,
    orch: Orchestrator,
    root: String,
    conn: ConnId,
    announced: bool,
    vars: RunVars,
}

struct Pending {
    plan: Plan,
    ids: Vec<String>,
    token: u64,
}

struct Run {
    id: u64,
    plan: Plan,
    launched: BTreeMap<String, bool>,
}

#[derive(Default)]
pub(crate) struct TaskState {
    runs: Vec<Run>,
    next_run: u64,
    next_token: u64,
    pending: HashMap<ConnId, Pending>,
    owners: HashMap<TermId, (u64, String)>,
}

fn settings_config() -> BTreeMap<String, String> {
    xenon_store::load_settings()
        .ok()
        .and_then(|settings| serde_json::to_value(settings).ok())
        .map(|value| shared::flatten_config(&value))
        .unwrap_or_default()
}

impl Phone {
    pub(super) fn run_task(&self, conn: ConnId, workspace_id: &str, label: String) {
        let started = (|| -> Result<(), String> {
            let (ws, root) = Self::workspace_root(workspace_id)?;
            let loaded = shared::load_tasks(&root)?;
            let shared::RunSetup { orch, ids, secrets } = shared::plan_run(&loaded.set, &label)?;
            let plan = Plan {
                ws,
                set: loaded.set,
                folder: loaded.folder,
                orch,
                root: label,
                conn,
                announced: false,
                vars: RunVars {
                    editor: None,
                    config: settings_config(),
                    inputs: BTreeMap::new(),
                    secrets,
                },
            };
            let mut st = locked(&self.tasks);
            st.pending.insert(
                conn,
                Pending {
                    plan,
                    ids,
                    token: 0,
                },
            );
            ask_next(self, &mut st, conn);
            Ok(())
        })();
        if let Err(error) = started {
            let msg = format!("{} \u{2014} {error}", shared::COULDNT_RUN);
            self.send_to(conn, ServerMsg::Error { msg });
        }
    }

    pub(super) fn task_answer(&self, conn: ConnId, token: u64, value: Option<Secret>) {
        let mut st = locked(&self.tasks);
        let Some(pending) = st.pending.get_mut(&conn).filter(|p| p.token == token) else {
            return;
        };
        match value {
            Some(value) if !pending.ids.is_empty() => {
                let id = pending.ids.remove(0);
                pending.plan.vars.inputs.insert(id, value.0);
                ask_next(self, &mut st, conn);
            }
            _ => {
                st.pending.remove(&conn);
            }
        }
    }

    pub(super) fn task_gone(&self, conn: ConnId) {
        locked(&self.tasks).pending.remove(&conn);
    }
}

fn ask_next(phone: &Phone, st: &mut TaskState, conn: ConnId) {
    st.next_token += 1;
    let token = st.next_token;
    let Some(pending) = st.pending.get_mut(&conn) else {
        return;
    };
    pending.token = token;
    let question = pending
        .ids
        .first()
        .map(|id| pending.plan.set.inputs.get(id).and_then(TaskQuestion::of));
    match question {
        None => {
            if let Some(p) = st.pending.remove(&conn) {
                launch_plan(phone, st, p.plan);
            }
        }
        Some(Some(question)) => phone.send_to(conn, ask(token, &question)),
        Some(None) => {}
    }
}

fn ask(token: u64, question: &TaskQuestion) -> ServerMsg {
    let (kind, options) = match &question.kind {
        QuestionKind::Text { password: true } => (TaskInputKind::Password, Vec::new()),
        QuestionKind::Text { password: false } => (TaskInputKind::Text, Vec::new()),
        QuestionKind::Pick { options } => (
            TaskInputKind::Pick,
            options
                .iter()
                .map(|o| TaskOptionWire {
                    label: o.label.clone(),
                    value: o.value.clone(),
                })
                .collect(),
        ),
    };
    ServerMsg::TaskInput {
        token,
        id: question.id.clone(),
        description: question.description.clone(),
        default: question.default.clone(),
        kind,
        options,
    }
}

fn launch_plan(phone: &Phone, st: &mut TaskState, plan: Plan) {
    st.next_run += 1;
    let id = st.next_run;
    let mut run = Run {
        id,
        plan,
        launched: BTreeMap::new(),
    };
    let actions = run.plan.orch.start();
    st.runs.push(run);
    apply_actions(phone, st, id, actions);
}

fn run_of(st: &mut TaskState, id: u64) -> Option<&mut Run> {
    st.runs.iter_mut().find(|r| r.id == id)
}

fn apply_actions(phone: &Phone, st: &mut TaskState, run_id: u64, actions: Vec<Action>) {
    let mut queue: std::collections::VecDeque<Action> = actions.into();
    while let Some(Action::Launch(label)) = queue.pop_front() {
        let Some(run) = run_of(st, run_id) else {
            return;
        };
        let composite = run
            .plan
            .set
            .get(&label)
            .is_some_and(|d| d.command.is_none());
        let satisfied = if composite {
            true
        } else {
            match launch_one(phone, st, run_id, &label) {
                Ok(background) => background,
                Err(error) => {
                    fail_run(phone, st, run_id, &label, &error);
                    return;
                }
            }
        };
        if satisfied && let Some(run) = run_of(st, run_id) {
            queue.extend(run.plan.orch.satisfied(&label));
        }
    }
    finish_if_done(st, run_id);
}

fn launch_one(phone: &Phone, st: &mut TaskState, run_id: u64, label: &str) -> Result<bool, String> {
    let run = run_of(st, run_id).ok_or("run is gone")?;
    let plan = &run.plan;
    let def = plan.set.get(label).ok_or("task is gone")?;
    let resolved = resolve_for_run(&plan.set, &plan.folder, def, &plan.vars)?;
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
    let (ws, conn, root) = (plan.ws, plan.conn, plan.root.clone());
    let announce = !plan.announced;
    let id = phone.terms.open(ws, spawn);
    let serial = phone.serial(id);
    st.owners.insert(id, (run_id, label.to_string()));
    if let Some(run) = run_of(st, run_id) {
        run.plan.announced = true;
        run.launched
            .insert(label.to_string(), resolved.is_background);
    }
    if announce {
        phone.send_to(
            conn,
            ServerMsg::TaskStarted {
                workspace_id: ws.to_string(),
                label: root,
                tab_id: serial,
            },
        );
    }
    Ok(resolved.is_background)
}

fn fail_run(phone: &Phone, st: &mut TaskState, run_id: u64, label: &str, reason: &str) {
    let Some(run) = run_of(st, run_id) else {
        return;
    };
    run.plan.orch.failed(label, reason);
    let (title, detail) = shared::failure_toast(&run.plan.root, label, reason);
    let conn = run.plan.conn;
    phone.send_to(
        conn,
        ServerMsg::Error {
            msg: format!("{title} \u{2014} {detail}"),
        },
    );
    finish_if_done(st, run_id);
}

fn finish_if_done(st: &mut TaskState, run_id: u64) {
    let done = run_of(st, run_id).is_some_and(|r| r.plan.orch.outcome().is_some());
    if done {
        st.runs.retain(|r| r.id != run_id);
    }
}

pub(super) fn exited(phone: &Phone, id: TermId, exit: Option<Exit>) {
    let mut close = None;
    {
        let mut st = locked(&phone.tasks);
        let Some((run_id, label)) = st.owners.remove(&id) else {
            return;
        };
        let Some(run) = run_of(&mut st, run_id) else {
            return;
        };
        let background = run.launched.get(&label).copied().unwrap_or(false);
        let close_when_done = run
            .plan
            .set
            .get(&label)
            .is_some_and(|d| d.presentation.close);
        let code = match exit {
            Some(Exit::Code(code)) => Some(code),
            _ => None,
        };
        if code == Some(0) {
            close = close_when_done.then_some(id);
            let actions = run.plan.orch.satisfied(&label);
            apply_actions(phone, &mut st, run_id, actions);
        } else if !background {
            fail_run(phone, &mut st, run_id, &label, &shared::exit_reason(code));
        }
    }
    if let Some(id) = close {
        phone.terms.close(id);
    }
}
