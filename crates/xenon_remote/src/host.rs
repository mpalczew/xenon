use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

use async_channel::Sender;

use crate::protocol::{ClientMsg, PairResponse, ServerMsg, TaskInfo, TerminalInfo, WorkspaceInfo};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

#[derive(Clone)]
pub struct Outbox {
    sender: SyncSender<ServerMsg>,
    wake: Option<std::sync::Arc<mio::Waker>>,
}

impl Outbox {
    pub(crate) const CAPACITY: usize = 16;

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

pub trait InputSink: Send + Sync {
    fn send(&self, msg: &ClientMsg) -> bool;
}

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
    Rejected,
    Storage(String),
}

pub enum HostRequest {
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
    ListTerminals {
        workspace_id: String,
        reply: SyncSender<Result<Vec<TerminalInfo>, String>>,
    },
    ListTasks {
        workspace_id: String,
        reply: SyncSender<Result<Vec<TaskInfo>, String>>,
    },
    CreateTerminal {
        workspace_id: String,
        reply: SyncSender<Result<TerminalInfo, String>>,
    },
    CloseTerminal {
        workspace_id: String,
        tab_id: u64,
        reply: SyncSender<Result<(), String>>,
    },
    Connected {
        conn: ConnId,
        device_id: String,
        out: Outbox,
        input: InputRoute,
    },
    Message {
        conn: ConnId,
        msg: ClientMsg,
    },
    InputSent {
        conn: ConnId,
    },
    Disconnected {
        conn: ConnId,
    },
}

pub(crate) type HostTx = Sender<HostRequest>;
