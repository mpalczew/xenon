//! Tasks from the phone: list a workspace's tasks.json and run one through the
//! same `start_task` path as the Mac's picker. Questions (`inputs`) go to the
//! phone that started the run; the run's terminal is an ordinary tab the phone
//! can attach to. Runs never move the Mac's tab or focus.

use std::rc::Rc;

use xenon_core::{GroupKind, TaskDef};
use xenon_remote::{ServerMsg, TaskInfo, TaskInputKind, TaskOptionWire};

use super::host::parse_workspace_id;
use super::*;
use crate::app::task_runs::{Origin, RemoteOrigin, RunRequest};
use crate::app::tasks::{row_command, with_preview_ctx};
use crate::task_input::{QuestionKind, TaskQuestion};

impl XenonApp {
    pub(super) fn list_remote_tasks(
        &mut self,
        workspace_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<Vec<TaskInfo>, String> {
        let wid = parse_workspace_id(workspace_id)?;
        self.ensure_workspace_live(wid, cx)?;
        let loaded = self.load_tasks(wid)?;
        Ok(with_preview_ctx(&loaded.set, &loaded.folder, |ctx| {
            loaded
                .set
                .tasks
                .iter()
                .filter(|t| !t.hide)
                .map(|t| task_info(t, row_command(t, ctx)))
                .collect()
        }))
    }

    /// `runTask` from a phone: same resolve, orchestration, and presentation as
    /// the Mac. A refusal to start goes back as `error`.
    pub(super) fn remote_run_task(
        &mut self,
        conn: ConnId,
        workspace_id: String,
        label: String,
        cx: &mut Context<Self>,
    ) {
        let Some(out) = self.conn_mut(conn).map(|c| c.out.clone()) else {
            return;
        };
        let started = (|| {
            let workspace = parse_workspace_id(&workspace_id)?;
            self.ensure_workspace_live(workspace, cx)?;
            let ask_out = out.clone();
            let started_out = out.clone();
            let ws_for_started = workspace_id.clone();
            let origin = Origin::Remote(RemoteOrigin {
                conn: conn.0,
                ask: Rc::new(move |token, question| {
                    ask_out.try_send(question_msg(token, question));
                }),
                started: Rc::new(move |label, tab| {
                    started_out.try_send(ServerMsg::TaskStarted {
                        workspace_id: ws_for_started.clone(),
                        label: label.to_string(),
                        tab_id: tab.0,
                    });
                }),
            });
            self.start_task(
                RunRequest {
                    workspace,
                    label,
                    new_tab: false,
                    origin,
                },
                cx,
            )
        })();
        if let Err(msg) = started {
            out.try_send(ServerMsg::Error { msg });
        }
    }
}

fn task_info(task: &TaskDef, command: String) -> TaskInfo {
    TaskInfo {
        label: task.label.clone(),
        detail: task.detail.clone(),
        command,
        group: task.group.map(|g| match g.kind {
            GroupKind::Build => "build".to_string(),
            GroupKind::Test => "test".to_string(),
        }),
        default: task.group.is_some_and(|g| g.is_default),
        unsupported: task.unsupported.clone(),
    }
}

fn question_msg(token: u64, q: TaskQuestion) -> ServerMsg {
    let (kind, options) = match q.kind {
        QuestionKind::Text { password: true } => (TaskInputKind::Password, Vec::new()),
        QuestionKind::Text { password: false } => (TaskInputKind::Text, Vec::new()),
        QuestionKind::Pick { options } => (
            TaskInputKind::Pick,
            options
                .into_iter()
                .map(|o| TaskOptionWire {
                    label: o.label,
                    value: o.value,
                })
                .collect(),
        ),
    };
    ServerMsg::TaskInput {
        token,
        id: q.id,
        description: q.description,
        default: q.default,
        kind,
        options,
    }
}
