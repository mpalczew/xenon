use std::sync::mpsc::{Receiver, SyncSender};

use async_channel::Sender;

use crate::protocol::{ClientMsg, PairResponse, ServerMsg, TaskInfo, TerminalInfo, WorkspaceInfo};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

#[derive(Clone)]
pub struct Outbox(SyncSender<ServerMsg>);

impl Outbox {
    pub(crate) const CAPACITY: usize = 16;

    pub fn try_send(&self, msg: ServerMsg) -> bool {
        self.0.try_send(msg).is_ok()
    }

    pub fn channel() -> (Self, Receiver<ServerMsg>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(Self::CAPACITY);
        (Self(tx), rx)
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
    },
    Message {
        conn: ConnId,
        msg: ClientMsg,
    },
    Disconnected {
        conn: ConnId,
    },
}

pub(crate) type HostTx = Sender<HostRequest>;
