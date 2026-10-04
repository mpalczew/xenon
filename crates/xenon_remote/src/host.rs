//! Requests from the remote server threads into the UI host (GPUI app).

use std::sync::mpsc::{SyncSender, TrySendError};

use async_channel::Sender;

use crate::protocol::{ClientMsg, PairResponse, ServerMsg, TerminalInfo, WorkspaceInfo};

/// One WebSocket connection, unique for the server's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// Mac → phone queue for one connection. Bounded: a slow phone drops patches
/// (the host then sends a full frame) instead of growing memory.
#[derive(Clone)]
pub struct Outbox(pub(crate) SyncSender<ServerMsg>);

impl Outbox {
    /// Bound on queued messages per connection.
    pub const CAPACITY: usize = 16;

    /// False when the queue is full or the connection is gone.
    pub fn try_send(&self, msg: ServerMsg) -> bool {
        match self.0.try_send(msg) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
        }
    }

    /// Test/host helper: a detached outbox plus its receiving end.
    pub fn channel() -> (Self, std::sync::mpsc::Receiver<ServerMsg>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(Self::CAPACITY);
        (Self(tx), rx)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairError {
    /// Wrong, used, or expired code.
    Rejected,
    Storage(String),
}

/// Work the server needs the GPUI app to do. Replies go over `reply`;
/// streaming output for a connection goes to its `Outbox`.
pub enum HostRequest {
    /// Device token → device id (None = unknown / revoked).
    Authenticate {
        token: String,
        reply: SyncSender<Option<String>>,
    },
    Pair {
        code: String,
        label: String,
        reply: SyncSender<Result<PairResponse, PairError>>,
    },
    ListWorkspaces {
        reply: SyncSender<Vec<WorkspaceInfo>>,
    },
    /// Reopens a closed workspace (without changing the Mac's active one).
    ListTerminals {
        workspace_id: String,
        reply: SyncSender<Result<Vec<TerminalInfo>, String>>,
    },
    /// Open a shell in that workspace. Does not change the Mac's screen.
    CreateTerminal {
        workspace_id: String,
        reply: SyncSender<Result<TerminalInfo, String>>,
    },
    /// Close one terminal in that workspace. The Mac follows only if it was showing it.
    CloseTerminal {
        workspace_id: String,
        tab_id: u64,
        reply: SyncSender<Result<(), String>>,
    },
    /// Authenticated WebSocket opened. Host sends `Ready` through `out`.
    Connected {
        conn: ConnId,
        device_id: String,
        out: Outbox,
    },
    /// A post-hello client message on `conn`.
    Message {
        conn: ConnId,
        msg: ClientMsg,
    },
    Disconnected {
        conn: ConnId,
    },
}

pub type HostTx = Sender<HostRequest>;
