use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use xenon_core::TermId;

use crate::Size;
use crate::holder_link::HolderLink;
use crate::proto::v1::ToHolder;
use crate::proto::{self, Frame, Frozen};

pub(crate) enum Call {
    Attach(Size),
    Claim(Size),
    Input(Vec<u8>),
    Seen,
    Close,
}

impl Call {
    fn frames(self, speaks: bool) -> Vec<Frame> {
        let control = |msg: &ToHolder| Frame::Control(proto::encode(msg));
        match self {
            Self::Close => vec![Frame::Control(proto::encode(&Frozen::Close))],
            _ if !speaks => Vec::new(),
            Self::Attach(size) => vec![
                control(&ToHolder::Claim { size, label: None }),
                control(&ToHolder::View { answers: true }),
            ],
            Self::Claim(size) => vec![control(&ToHolder::Claim { size, label: None })],
            Self::Input(bytes) => vec![Frame::Data(bytes)],
            Self::Seen => vec![control(&ToHolder::Seen)],
        }
    }
}

#[derive(Default)]
struct Early {
    size: Option<Size>,
    attach: bool,
    input: Vec<u8>,
    close: bool,
}

impl Early {
    fn fold(&mut self, call: Call) {
        match call {
            Call::Attach(size) => {
                self.size = Some(size);
                self.attach = true;
            }
            Call::Claim(size) => self.size = Some(size),
            Call::Input(bytes) => self.input.extend_from_slice(&bytes),
            Call::Seen => {}
            Call::Close => self.close = true,
        }
    }

    fn replay(self) -> Vec<Call> {
        let mut calls = Vec::new();
        match (self.size, self.attach) {
            (Some(size), true) => calls.push(Call::Attach(size)),
            (Some(size), false) => calls.push(Call::Claim(size)),
            (None, _) => {}
        }
        if !self.input.is_empty() {
            calls.push(Call::Input(self.input));
        }
        if self.close {
            calls.push(Call::Close);
        }
        calls
    }
}

enum Slot {
    Waiting {
        early: Early,
        connecting: bool,
    },
    Live {
        link: Arc<HolderLink>,
        out: Sender<Call>,
    },
}

#[derive(Default)]
pub(crate) struct Links(Mutex<HashMap<TermId, Slot>>);

impl Links {
    fn map(&self) -> MutexGuard<'_, HashMap<TermId, Slot>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn expect(&self, id: TermId) -> bool {
        let mut map = self.map();
        if map.contains_key(&id) {
            return false;
        }
        map.insert(
            id,
            Slot::Waiting {
                early: Early::default(),
                connecting: false,
            },
        );
        true
    }

    pub(crate) fn send(&self, id: TermId, call: Call) -> bool {
        match self.map().get_mut(&id) {
            Some(Slot::Live { out, .. }) => {
                let _ = out.send(call);
                true
            }
            Some(Slot::Waiting { early, .. }) => {
                early.fold(call);
                true
            }
            None => false,
        }
    }

    pub(crate) fn begin_connect(&self, id: TermId) -> bool {
        match self.map().get_mut(&id) {
            Some(Slot::Waiting { connecting, .. }) if !*connecting => {
                *connecting = true;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn connect_failed(&self, id: TermId) {
        let mut map = self.map();
        if matches!(map.get(&id), Some(Slot::Waiting { .. })) {
            map.remove(&id);
        }
    }

    pub(crate) fn publish(&self, link: Arc<HolderLink>, stream: UnixStream) {
        let (out, queue) = mpsc::channel();
        let mut map = self.map();
        if let Some(Slot::Waiting { early, .. }) = map.remove(&link.id) {
            for call in early.replay() {
                let _ = out.send(call);
            }
        }
        let speaks = link.speaks;
        thread::spawn(move || write(&queue, stream, speaks));
        map.insert(link.id, Slot::Live { link, out });
    }

    pub(crate) fn remove(&self, link: &Arc<HolderLink>) -> bool {
        let mut map = self.map();
        let ours = matches!(
            map.get(&link.id),
            Some(Slot::Live { link: l, .. }) if Arc::ptr_eq(l, link)
        );
        if ours {
            map.remove(&link.id);
        }
        ours
    }

    pub(crate) fn get(&self, id: TermId) -> Option<Arc<HolderLink>> {
        match self.map().get(&id) {
            Some(Slot::Live { link, .. }) => Some(Arc::clone(link)),
            _ => None,
        }
    }

    pub(crate) fn live(&self) -> Vec<Arc<HolderLink>> {
        self.map()
            .values()
            .filter_map(|slot| match slot {
                Slot::Live { link, .. } => Some(Arc::clone(link)),
                Slot::Waiting { .. } => None,
            })
            .collect()
    }
}

fn write(queue: &mpsc::Receiver<Call>, mut stream: UnixStream, speaks: bool) {
    while let Ok(call) = queue.recv() {
        for frame in call.frames(speaks) {
            let written = match frame {
                Frame::Control(body) => proto::write_raw_control(&mut stream, &body),
                Frame::Data(bytes) => proto::write_data(&mut stream, &bytes),
            };
            if written.is_err() {
                return;
            }
        }
    }
}
