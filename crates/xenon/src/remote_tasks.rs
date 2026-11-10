use std::rc::Rc;

use xenon_core::tasks::{self as shared, QuestionKind, TaskQuestion};
use xenon_core::{TabId, WorkspaceId};
use xenon_remote::{
    ConnId, Secret, ServerMsg, TaskInfo, TaskInputKind, TaskOptionWire, parse_workspace_id,
};

use crate::app::App;
use crate::tasks::Start;

impl App {
    pub(crate) fn remote_list_tasks(
        self: &Rc<Self>,
        workspace_id: &str,
    ) -> Result<Vec<TaskInfo>, String> {
        let ws = parse_workspace_id(workspace_id)?;
        self.remote_live(ws)?;
        let loaded = self.load_workspace_tasks(ws)?;
        Ok(shared::task_rows(&loaded.set, &loaded.folder)
            .into_iter()
            .map(|row| TaskInfo {
                label: row.label,
                detail: row.detail,
                command: row.command_line,
                group: row.group.map(str::to_string),
                default: row.is_default,
                unsupported: row.unsupported,
            })
            .collect())
    }

    pub(crate) fn remote_run_task(
        self: &Rc<Self>,
        conn: ConnId,
        workspace_id: &str,
        label: String,
    ) {
        let started = parse_workspace_id(workspace_id).and_then(|ws| {
            self.remote_live(ws)?;
            self.start_task(
                ws,
                label,
                Start {
                    quiet: true,
                    origin: Some(conn),
                    ..Start::default()
                },
            )
        });
        if let Err(error) = started {
            let msg = format!("{} \u{2014} {error}", shared::COULDNT_RUN);
            self.send_to(conn, ServerMsg::Error { msg });
        }
    }

    pub(crate) fn remote_ask_input(self: &Rc<Self>, conn: ConnId, question: &TaskQuestion) {
        let token = self.tasks.next_token.get() + 1;
        self.tasks.next_token.set(token);
        *self.tasks.phone_ask.borrow_mut() = Some((token, conn));
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
        self.send_to(
            conn,
            ServerMsg::TaskInput {
                token,
                id: question.id.clone(),
                description: question.description.clone(),
                default: question.default.clone(),
                kind,
                options,
            },
        );
    }

    /// The phone's answer (`None` cancels the run). Only the connection that
    /// was asked, with the current token, counts.
    pub(crate) fn remote_task_answer(
        self: &Rc<Self>,
        conn: ConnId,
        token: u64,
        value: Option<Secret>,
    ) {
        let asked = *self.tasks.phone_ask.borrow();
        if asked != Some((token, conn)) {
            return;
        }
        *self.tasks.phone_ask.borrow_mut() = None;
        self.answer_task_input(value.map(|v| v.0));
    }

    pub(crate) fn remote_task_gone(self: &Rc<Self>, conn: ConnId) {
        let asked = self.tasks.phone_ask.borrow().filter(|(_, c)| *c == conn);
        if let Some((token, _)) = asked {
            self.remote_task_answer(conn, token, None);
        }
    }

    pub(crate) fn remote_task_started(
        &self,
        conn: ConnId,
        ws: WorkspaceId,
        label: &str,
        tab: TabId,
    ) {
        self.send_to(
            conn,
            ServerMsg::TaskStarted {
                workspace_id: ws.to_string(),
                label: label.to_string(),
                tab_id: tab.0,
            },
        );
    }
}
