use std::path::PathBuf;

use crate::remote::{
    PairError, PairResponse, TaskInfo, TerminalInfo, WorkspaceInfo, parse_workspace_id, tilde,
    wire_dot,
};
use xenon_core::tasks as shared;
use xenon_core::{Registry, WorkspaceId};

use super::{Phone, locked};
use xenon_term::{Spawn, TermInfo};

impl Phone {
    pub(crate) fn authenticate(&self, token: &str) -> Option<String> {
        locked(&self.server)
            .rt
            .as_mut()
            .and_then(|rt| rt.devices.authenticate_touch(token))
    }

    pub(crate) fn pair(&self, code: &str, label: &str) -> Result<PairResponse, PairError> {
        let reply = {
            let mut guard = locked(&self.server);
            let rt = guard.rt.as_mut().ok_or(PairError::Rejected)?;
            let host = rt.host_name.clone();
            rt.devices.pair_response(code, label, &host)?
        };
        self.changed();
        Ok(reply)
    }

    pub(crate) fn workspaces(&self) -> Vec<WorkspaceInfo> {
        let registry = xenon_store::load_registry().unwrap_or_default();
        crate::remote::list_workspaces(&registry, |id| self.workspace_summary(id))
    }

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

    fn terminal_info(&self, term: &TermInfo) -> TerminalInfo {
        TerminalInfo {
            tab_id: self.serial(term.id),
            title: Some(term.status.title.clone()),
            cwd: tilde(std::path::Path::new(&term.cwd)),
            dot: xenon_core::workspace_dot(term.status.working, term.status.attention)
                .map(wire_dot),
        }
    }

    pub(crate) fn terminals(&self, workspace_id: &str) -> Result<Vec<TerminalInfo>, String> {
        let (ws, _) = Self::workspace_root(workspace_id)?;
        let mut terms: Vec<_> = self
            .terms
            .list()
            .into_iter()
            .filter(|t| t.workspace == ws)
            .map(|t| (self.serial(t.id), t))
            .collect();
        terms.sort_by_key(|(serial, _)| *serial);
        Ok(terms.iter().map(|(_, t)| self.terminal_info(t)).collect())
    }

    pub(crate) fn new_terminal(&self, workspace_id: &str) -> Result<TerminalInfo, String> {
        let (ws, root) = Self::workspace_root(workspace_id)?;
        let id = self.terms.open(ws, Spawn::shell(&root));
        if !self.terms.wait_ready(id, std::time::Duration::from_secs(5)) {
            return Err("could not open a terminal".into());
        }
        let info = self.terms.info(id).ok_or("could not open a terminal")?;
        Ok(self.terminal_info(&info))
    }

    pub(super) fn find(&self, ws: WorkspaceId, serial: u64) -> Option<TermInfo> {
        self.terms
            .list()
            .into_iter()
            .find(|t| t.workspace == ws && self.serial(t.id) == serial)
    }

    pub(crate) fn close_terminal(&self, workspace_id: &str, tab_id: u64) -> Result<(), String> {
        let ws = parse_workspace_id(workspace_id)?;
        let found = self.find(ws, tab_id).ok_or("not found")?;
        self.terms.close(found.id);
        Ok(())
    }

    pub(crate) fn close_workspace(workspace_id: &str) -> Result<(), String> {
        let workspace = parse_workspace_id(workspace_id)?;
        match xenon_store::try_handoff_request(&xenon_store::IpcRequest::CloseWorkspace {
            workspace,
        }) {
            xenon_store::Handoff::Sent(_) => Ok(()),
            xenon_store::Handoff::Rejected(error) => Err(error),
            xenon_store::Handoff::NoServer => {
                Err("Open Xenon on the computer to close this workspace".into())
            }
        }
    }

    pub(crate) fn list_tasks(&self, workspace_id: &str) -> Result<Vec<TaskInfo>, String> {
        let (_, root) = Self::workspace_root(workspace_id)?;
        if shared::find_tasks_json(&root).is_none() {
            return Ok(Vec::new());
        }
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
