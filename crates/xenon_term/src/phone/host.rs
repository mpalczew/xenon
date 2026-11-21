//! Answers to the HTTP side: workspaces, terminals, tasks, pairing and sessions.
//! Workspace names and roots come from the registry file the window writes; the service
//! only reads it.

use std::path::PathBuf;

use xenon_core::tasks as shared;
use xenon_core::{Registry, WorkspaceId};
use xenon_remote::{
    HostRequest, PairError, PairResponse, TaskInfo, TerminalInfo, WorkspaceInfo,
    parse_workspace_id, tilde, wire_dot,
};

use super::{Phone, locked};
use crate::Spawn;
use crate::term::Term;

impl Phone {
    pub(super) fn request(&self, req: HostRequest) {
        match req {
            HostRequest::Authenticate { token, reply } => {
                let id = locked(&self.server)
                    .rt
                    .as_mut()
                    .and_then(|rt| rt.devices.authenticate_touch(&token));
                let _ = reply.send(id);
            }
            HostRequest::Pair { code, label, reply } => {
                let _ = reply.send(self.pair(&code, &label));
            }
            HostRequest::ListWorkspaces { reply } => {
                let _ = reply.send(self.workspaces());
            }
            HostRequest::ListTerminals {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.terminals(&workspace_id));
            }
            HostRequest::ListTasks {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.list_tasks(&workspace_id));
            }
            HostRequest::CreateTerminal {
                workspace_id,
                reply,
            } => {
                let _ = reply.send(self.new_terminal(&workspace_id));
            }
            HostRequest::CloseTerminal {
                workspace_id,
                tab_id,
                reply,
            } => {
                let _ = reply.send(self.close_terminal(&workspace_id, tab_id));
            }
            HostRequest::Connected {
                conn,
                device_id,
                out,
            } => self.connected(conn, device_id, out),
            HostRequest::Message { conn, msg } => self.message(conn, msg),
            HostRequest::Disconnected { conn } => self.disconnected(conn),
        }
    }

    fn pair(&self, code: &str, label: &str) -> Result<PairResponse, PairError> {
        let reply = {
            let mut guard = locked(&self.server);
            let rt = guard.rt.as_mut().ok_or(PairError::Rejected)?;
            let host = rt.host_name.clone();
            rt.devices.pair_response(code, label, &host)?
        };
        self.changed();
        Ok(reply)
    }

    fn workspaces(&self) -> Vec<WorkspaceInfo> {
        let registry = xenon_store::load_registry().unwrap_or_default();
        xenon_remote::list_workspaces(&registry, |id| self.workspace_summary(id))
    }

    /// The workspace's root, open or recently closed.
    fn root_of(registry: &Registry, ws: WorkspaceId) -> Result<PathBuf, String> {
        registry
            .workspace(ws)
            .map(|r| r.root.clone())
            .or_else(|| {
                registry
                    .closed_workspaces
                    .iter()
                    .find(|r| r.id == ws)
                    .map(|r| r.root.clone())
            })
            .ok_or_else(|| "unknown workspace".to_string())
    }

    pub(super) fn workspace_root(workspace_id: &str) -> Result<(WorkspaceId, PathBuf), String> {
        let ws = parse_workspace_id(workspace_id)?;
        let registry = xenon_store::load_registry().unwrap_or_default();
        Ok((ws, Self::root_of(&registry, ws)?))
    }

    fn terminal_info(term: &Term) -> TerminalInfo {
        let status = term.info().status;
        TerminalInfo {
            tab_id: term.serial,
            title: Some(status.title.clone()),
            cwd: tilde(&term.cwd),
            active: false,
            dot: xenon_core::workspace_dot(status.working, status.attention).map(wire_dot),
        }
    }

    fn terminals(&self, workspace_id: &str) -> Result<Vec<TerminalInfo>, String> {
        let (ws, _) = Self::workspace_root(workspace_id)?;
        let mut terms: Vec<_> = self
            .terms
            .all()
            .into_iter()
            .filter(|t| t.workspace == ws)
            .collect();
        terms.sort_by_key(|t| t.serial);
        Ok(terms.iter().map(|t| Self::terminal_info(t)).collect())
    }

    /// Makes the shell; the window adopts it as an unselected tab.
    fn new_terminal(&self, workspace_id: &str) -> Result<TerminalInfo, String> {
        let (ws, root) = Self::workspace_root(workspace_id)?;
        let id = self
            .terms
            .create(ws, &Spawn::shell(&root), None)
            .map_err(|e| format!("could not open a terminal: {e}"))?;
        let term = self.terms.get(id).ok_or("could not open a terminal")?;
        Ok(Self::terminal_info(&term))
    }

    fn close_terminal(&self, workspace_id: &str, tab_id: u64) -> Result<(), String> {
        let ws = parse_workspace_id(workspace_id)?;
        let found = self
            .terms
            .all()
            .into_iter()
            .find(|t| t.workspace == ws && t.serial == tab_id)
            .ok_or("not found")?;
        self.terms.close(found.id);
        Ok(())
    }

    fn list_tasks(&self, workspace_id: &str) -> Result<Vec<TaskInfo>, String> {
        let (_, root) = Self::workspace_root(workspace_id)?;
        let loaded = shared::load_tasks(&root)?;
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
}
