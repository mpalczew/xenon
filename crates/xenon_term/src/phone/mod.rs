//! The phone server inside the service: HTTP and WebSocket from `xenon_remote`, answered
//! here from `Terms`. Phones work with no window open.

pub(crate) mod colors;
mod dots;
mod frames;
mod host;
mod server;
mod session;
mod status;
mod tasks;
mod watch;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use xenon_remote::{ConnId, DeviceBook, HostRequest, Reachability, RemoteServer};

use crate::hub::Note;
use crate::phone_api::{PhoneCall, PhoneReply};
use crate::terms::Terms;

pub(crate) use session::Conn;
pub(crate) use watch::Watch;

type Listener = Box<dyn Fn() + Send + Sync>;

/// Phone connection ids share `Claims` with window ids; this bit keeps them apart.
pub(crate) const PHONE_BIT: u64 = 1 << 63;

pub(crate) struct Runtime {
    pub _server: RemoteServer,
    pub port: u16,
    pub reach: Reachability,
    pub host_name: String,
    pub devices: DeviceBook,
}

#[derive(Default)]
pub(crate) struct Server {
    pub rt: Option<Runtime>,
    /// The port is busy (the other Xenon has it): retrying.
    pub waiting: bool,
}

pub(crate) struct Phone {
    pub terms: Arc<Terms>,
    pub server: Mutex<Server>,
    pub conns: Mutex<HashMap<ConnId, Conn>>,
    pub tasks: Mutex<tasks::TaskState>,
    me: Weak<Phone>,
    tx: async_channel::Sender<HostRequest>,
    changes: Mutex<(u64, Vec<(u64, Listener)>)>,
}

pub(crate) fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Phone {
    pub(crate) fn start(terms: &Arc<Terms>) -> Arc<Self> {
        let (tx, rx) = async_channel::unbounded();
        let phone = Arc::new_cyclic(|me| Self {
            me: me.clone(),
            terms: Arc::clone(terms),
            server: Mutex::new(Server::default()),
            conns: Mutex::new(HashMap::new()),
            tasks: Mutex::new(tasks::TaskState::default()),
            tx,
            changes: Mutex::new((0, Vec::new())),
        });
        let weak = Arc::downgrade(&phone);
        terms.hub.subscribe(Box::new(move |note| {
            if let Some(phone) = weak.upgrade() {
                phone.on_note(note);
            }
        }));
        server::spawn_threads(&phone, rx);
        if xenon_store::load_settings()
            .unwrap_or_default()
            .remote_enabled
        {
            phone.start_server();
        }
        phone
    }

    /// Answer a window's phone call (JSON in, JSON out).
    pub(crate) fn call(&self, json: &str) -> String {
        let reply = match serde_json::from_str::<PhoneCall>(json) {
            Ok(PhoneCall::Status) => PhoneReply::Status {
                status: self.status(),
            },
            Ok(PhoneCall::SetEnabled { on }) => {
                self.set_enabled(on);
                PhoneReply::Status {
                    status: self.status(),
                }
            }
            Ok(PhoneCall::Pairing) => PhoneReply::Pairing {
                pairing: self.pairing(),
            },
            Ok(PhoneCall::Revoke { device }) => {
                self.revoke(device.as_deref());
                PhoneReply::Status {
                    status: self.status(),
                }
            }
            Err(e) => PhoneReply::Pairing {
                pairing: Err(e.to_string()),
            },
        };
        serde_json::to_string(&reply).unwrap_or_default()
    }

    pub(crate) fn on_change(&self, f: Listener) -> u64 {
        let mut guard = locked(&self.changes);
        guard.0 += 1;
        let id = guard.0;
        guard.1.push((id, f));
        id
    }

    pub(crate) fn off_change(&self, id: u64) {
        locked(&self.changes).1.retain(|(i, _)| *i != id);
    }

    /// Something a window shows about the phone server changed.
    pub(crate) fn changed(&self) {
        for (_, listener) in locked(&self.changes).1.iter() {
            listener();
        }
    }

    fn on_note(&self, note: &Note) {
        self.push_dots();
        match note {
            Note::Status { id, status } if status.exit.is_some() => {
                tasks::exited(self, *id, status.exit);
            }
            Note::Closed { id } => {
                tasks::exited(self, *id, None);
                self.terminal_closed(*id);
            }
            _ => {}
        }
    }

    /// A phone attached to this terminal is told it is gone.
    fn terminal_closed(&self, id: xenon_core::TermId) {
        let conns: Vec<ConnId> = locked(&self.conns)
            .iter()
            .filter(|(_, c)| c.attach.as_ref().is_some_and(|a| a.term.id == id))
            .map(|(conn, _)| *conn)
            .collect();
        for conn in conns {
            self.detach(conn);
            self.send_to(
                conn,
                xenon_remote::ServerMsg::Closed {
                    reason: xenon_remote::ClosedReason::TabClosed,
                },
            );
        }
    }

    pub(crate) fn self_ref(&self) -> Weak<Phone> {
        self.me.clone()
    }

    pub(crate) fn send_to(&self, conn: ConnId, msg: xenon_remote::ServerMsg) {
        if let Some(c) = locked(&self.conns).get(&conn) {
            c.out.try_send(msg);
        }
    }
}
