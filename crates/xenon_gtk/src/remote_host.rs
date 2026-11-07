//! What the phone asks the GTK app for: workspace and terminal lists, new and
//! closed terminals, pairing and sign-in. None of it changes the screen the
//! user is looking at.

use std::rc::Rc;

use xenon_core::{TabId, WorkspaceId};
use xenon_remote::{
    HostRequest, PairError, PairResponse, TaskInfo, TerminalInfo, WorkspaceInfo,
    parse_workspace_id, tilde, wire_dot,
};

use crate::app::App;
use crate::model::TabKind;

impl App {
    pub(crate) fn remote_request(self: &Rc<Self>, req: HostRequest) {
        match req {
            HostRequest::Authenticate { token, reply } => {
                let id = self
                    .remote
                    .rt
                    .borrow_mut()
                    .as_mut()
                    .and_then(|rt| rt.devices.authenticate_touch(&token));
                let _ = reply.send(id);
            }
            HostRequest::Pair { code, label, reply } => {
                let _ = reply.send(self.remote_pair(&code, &label));
            }
            HostRequest::ListWorkspaces { reply } => {
                let _ = reply.send(self.remote_workspaces());
            }
            HostRequest::ListTerminals {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.remote_terminals(&workspace_id));
            }
            HostRequest::ListTasks { reply, .. } => {
                let none: Result<Vec<TaskInfo>, String> = Ok(Vec::new());
                let _ = reply.send(none);
            }
            HostRequest::CreateTerminal {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.remote_new_terminal(&workspace_id));
            }
            HostRequest::CloseTerminal {
                workspace_id,
                tab_id,
                reply,
            } => {
                let _ = reply.send(self.remote_close_terminal(&workspace_id, tab_id));
            }
            HostRequest::Connected {
                conn,
                device_id,
                out,
                input,
            } => self.remote_connected(conn, device_id, out, input),
            HostRequest::Message { conn, msg } => self.remote_message(conn, msg),
            HostRequest::InputSent { .. } => {}
            HostRequest::Disconnected { conn } => self.remote_disconnected(conn),
        }
    }

    fn remote_pair(self: &Rc<Self>, code: &str, label: &str) -> Result<PairResponse, PairError> {
        let reply = {
            let mut rt = self.remote.rt.borrow_mut();
            let rt = rt.as_mut().ok_or(PairError::Rejected)?;
            let host = rt.host_name.clone();
            rt.devices.pair_response(code, label, &host)?
        };
        self.toast("Phone paired");
        self.remote_publish();
        Ok(reply)
    }

    fn remote_workspaces(&self) -> Vec<WorkspaceInfo> {
        let model = self.model.borrow();
        xenon_remote::list_workspaces(&model.registry, |id| {
            (model.workspace_dot(id), model.terminal_ids(id).len())
        })
    }

    /// Make the workspace live without showing it: reopen it from Recent when
    /// closed, then start its terminals.
    pub(crate) fn remote_live(self: &Rc<Self>, ws: WorkspaceId) -> Result<(), String> {
        let closed_root = {
            let model = self.model.borrow();
            model
                .registry
                .closed_workspaces
                .iter()
                .find(|r| r.id == ws)
                .map(|r| r.root.clone())
        };
        if let Some(root) = closed_root {
            self.model.borrow_mut().open_dir(&root);
            self.sidebar.rebuild(&self.model.borrow());
            self.save_registry();
        }
        if self.model.borrow().rec(ws).is_none() {
            return Err("unknown workspace".into());
        }
        if !self.model.borrow().drawable(ws) {
            return Err("SSH workspaces are not available".into());
        }
        self.model.borrow_mut().materialize(ws);
        self.sync(ws);
        Ok(())
    }

    fn remote_terminals(self: &Rc<Self>, workspace_id: &str) -> Result<Vec<TerminalInfo>, String> {
        let ws = parse_workspace_id(workspace_id)?;
        self.remote_live(ws)?;
        let model = self.model.borrow();
        let cwd = model.rec(ws).map(|r| tilde(&r.root)).unwrap_or_default();
        Ok(model
            .terminal_ids(ws)
            .into_iter()
            .map(|(tab, index)| terminal_info(&model, ws, tab, index, &cwd))
            .collect())
    }

    fn remote_new_terminal(self: &Rc<Self>, workspace_id: &str) -> Result<TerminalInfo, String> {
        let ws = parse_workspace_id(workspace_id)?;
        self.remote_live(ws)?;
        let before = self
            .model
            .borrow()
            .spaces
            .get(&ws)
            .and_then(|s| s.content.active_tab())
            .map(|t| t.id);
        let tab = self
            .model
            .borrow_mut()
            .new_terminal(ws)
            .ok_or("could not open a terminal")?;
        if let Some(before) = before {
            // The user keeps looking at what they were looking at.
            self.model.borrow_mut().select_tab(ws, before);
        }
        self.sync(ws);
        self.save_session(ws);
        let model = self.model.borrow();
        let cwd = model.rec(ws).map(|r| tilde(&r.root)).unwrap_or_default();
        let index = model
            .terminal_ids(ws)
            .into_iter()
            .find(|(t, _)| *t == tab)
            .map_or(0, |(_, i)| i);
        Ok(terminal_info(&model, ws, tab, index, &cwd))
    }

    fn remote_close_terminal(
        self: &Rc<Self>,
        workspace_id: &str,
        tab_id: u64,
    ) -> Result<(), String> {
        let ws = parse_workspace_id(workspace_id)?;
        let tab = TabId(tab_id);
        let found = self
            .model
            .borrow()
            .tab(ws, tab)
            .is_some_and(|t| matches!(t.kind, TabKind::Terminal));
        if !found {
            return Err("not found".into());
        }
        self.close_tab_ui(ws, tab);
        Ok(())
    }
}

fn terminal_info(
    model: &crate::model::Model,
    ws: WorkspaceId,
    tab: TabId,
    index: usize,
    cwd: &str,
) -> TerminalInfo {
    TerminalInfo {
        tab_id: tab.0,
        title: model.tab(ws, tab).map(|t| t.label(index)),
        cwd: cwd.to_string(),
        active: model
            .spaces
            .get(&ws)
            .and_then(|s| s.content.active_tab())
            .is_some_and(|t| t.id == tab),
        dot: model.tab_dot(ws, tab).map(wire_dot),
    }
}
