//! Requests from the remote server threads into the UI host (GPUI app).

use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

use async_channel::Sender;

use crate::protocol::{ClientMsg, PairResponse, ServerMsg, TerminalInfo, WorkspaceInfo};

/// One WebSocket connection, unique for the server's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// Mac → phone queue for one connection. Bounded: a slow phone drops patches
/// (the host then sends a full frame) instead of growing memory.
#[derive(Clone)]
pub struct Outbox {
    sender: SyncSender<ServerMsg>,
    wake: Option<std::sync::Arc<mio::Waker>>,
}

impl Outbox {
    /// Bound on queued messages per connection.
    pub const CAPACITY: usize = 16;

    /// False when the queue is full or the connection is gone.
    pub fn try_send(&self, msg: ServerMsg) -> bool {
        match self.sender.try_send(msg) {
            Ok(()) => {
                if let Some(wake) = &self.wake {
                    let _ = wake.wake();
                }
                true
            }
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
        }
    }

    pub(crate) fn wake_on_send(&mut self, wake: std::sync::Arc<mio::Waker>) {
        self.wake = Some(wake);
    }

    /// Test/host helper: a detached outbox plus its receiving end.
    pub fn channel() -> (Self, std::sync::mpsc::Receiver<ServerMsg>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(Self::CAPACITY);
        (
            Self {
                sender: tx,
                wake: None,
            },
            rx,
        )
    }
}

/// Input the host can deliver from the connection thread, without a trip
/// through the UI thread.
pub trait InputSink: Send + Sync {
    /// True when `msg` was fully handled here.
    fn send(&self, msg: &ClientMsg) -> bool;
}

/// Where a connection's fast-path input goes. The host fills it in when the
/// phone attaches to a terminal and clears it on detach; until then input
/// takes the normal `HostRequest::Message` path.
#[derive(Clone, Default)]
pub struct InputRoute(Arc<Mutex<Option<Arc<dyn InputSink>>>>);

impl InputRoute {
    pub fn set(&self, sink: Option<Arc<dyn InputSink>>) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = sink;
        }
    }

    pub(crate) fn get(&self) -> Option<Arc<dyn InputSink>> {
        self.0.lock().ok()?.clone()
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
        input: InputRoute,
    },
    /// A post-hello client message on `conn`.
    Message {
        conn: ConnId,
        msg: ClientMsg,
    },
    /// Input already delivered through the connection's `InputRoute`: the
    /// host only does its own bookkeeping (activity, attention).
    InputSent {
        conn: ConnId,
    },
    Disconnected {
        conn: ConnId,
    },
}

pub type HostTx = Sender<HostRequest>;
