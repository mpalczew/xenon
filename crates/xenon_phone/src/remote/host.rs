use std::sync::mpsc::{Receiver, SyncSender};

use crate::remote::protocol::ServerMsg;

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
