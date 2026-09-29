//! Status the phone and the sidebar show: workspace/tab dots and the footer.

use super::devices::device_short_name;
use super::*;
use xenon_remote::{Dot, ServerMsg};

/// What the sidebar footer shows while the remote is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RemoteFooter {
    Listening,
    Connected { device: String },
    Driving { device: String, workspace: String },
}

impl XenonApp {
    /// Push workspace + attached-tab dots to every phone whose view changed.
    pub(super) fn broadcast_remote_dots(&mut self, cx: &mut Context<Self>) {
        let Some(runtime) = self.services.remote.as_ref() else {
            return;
        };
        if runtime.conns.is_empty() {
            return;
        }
        let workspaces: BTreeMap<String, Dot> = self
            .registry
            .workspaces
            .iter()
            .filter_map(|w| {
                Some((
                    w.id.to_string(),
                    host::wire_dot(self.workspace_status(w.id, cx)?),
                ))
            })
            .collect();
        let ids: Vec<ConnId> = runtime.conns.keys().copied().collect();
        for id in ids {
            let (workspace, tabs) = self.attached_tab_dots(id, cx);
            let msg = ServerMsg::Dots {
                workspaces: workspaces.clone(),
                workspace,
                tabs,
            };
            let Some(c) = self.conn_mut(id) else {
                continue;
            };
            if c.last_dots.as_ref() != Some(&msg) && c.out.try_send(msg.clone()) {
                c.last_dots = Some(msg);
            }
        }
    }

    /// The attached workspace's id and its tab dots (none when not attached).
    fn attached_tab_dots(&self, conn: ConnId, cx: &App) -> (Option<String>, BTreeMap<u64, Dot>) {
        let Some(workspace) = self
            .conn(conn)
            .and_then(|c| c.attach.as_ref())
            .map(|a| a.workspace)
        else {
            return (None, BTreeMap::new());
        };
        (
            Some(workspace.to_string()),
            self.remote_terminal_dots(workspace, cx),
        )
    }

    /// Sidebar footer state (None while the remote is off).
    pub(crate) fn remote_footer(&self, cx: &App) -> Option<RemoteFooter> {
        let runtime = self.services.remote.as_ref()?;
        let driving = runtime.conns.values().find_map(|c| {
            let a = c.attach.as_ref()?;
            let fit = a
                .view
                .upgrade()?
                .read(cx)
                .active_phone_fit()?
                .device
                .to_string();
            let workspace = self.registry.workspace(a.workspace)?.name.clone();
            Some(RemoteFooter::Driving {
                device: fit,
                workspace,
            })
        });
        if driving.is_some() {
            return driving;
        }
        let first = runtime.conns.values().next().map(|c| {
            device_short_name(runtime.devices.label(&c.device_id).unwrap_or("Phone")).to_string()
        });
        Some(match (first, runtime.conns.len()) {
            (None, _) => RemoteFooter::Listening,
            (Some(device), 1) => RemoteFooter::Connected { device },
            (Some(_), n) => RemoteFooter::Connected {
                device: format!("{n} phones"),
            },
        })
    }
}
