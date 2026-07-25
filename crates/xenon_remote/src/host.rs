//! Requests from the remote server thread into the UI host.

use std::sync::mpsc::SyncSender;

use async_channel::Sender;

use crate::protocol::{TerminalInfo, WorkspaceInfo};

/// Viewport snapshot for one terminal tab (visible grid only).
#[derive(Clone, Debug)]
pub struct ViewportSnapshot {
    pub tab_id: u64,
    pub seq: u64,
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<String>,
}

/// Work the server needs the GPUI app to do.
pub enum HostRequest {
    ListWorkspaces {
        reply: SyncSender<Vec<WorkspaceInfo>>,
    },
    /// Activate / reopen a workspace so its terminals exist, then list them.
    ListTerminals {
        workspace_id: String,
        reply: SyncSender<Result<Vec<TerminalInfo>, String>>,
    },
    CaptureFrame {
        workspace_id: String,
        tab_id: u64,
        reply: SyncSender<Result<ViewportSnapshot, String>>,
    },
    Inject {
        workspace_id: String,
        tab_id: u64,
        text: String,
        reply: SyncSender<Result<(), String>>,
    },
}

pub type HostTx = Sender<HostRequest>;
