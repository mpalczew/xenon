mod api;
mod control;
mod dots;
mod frames;
mod host;
mod link;
mod look;
mod remote;
mod screen;
mod server;
mod session;
mod status;
mod tasks;
mod window;
mod worklist;

use std::collections::HashMap;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::remote::{ConnId, DeviceBook, Reachability, RemoteServer};
use xenon_core::TermId;
use xenon_term::{Event, Exit, Terminals};

pub use api::Pairing;
pub use link::PhoneLink;
pub use remote::{
    DeviceRow, KEEP_AWAKE_DETAIL, KEEP_AWAKE_LABEL, RemoteStatus, ago, effective_port,
    normalize_hostname,
};
pub(crate) use session::Conn;

type Listener = Box<dyn Fn() + Send + Sync>;

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
    pub problem: Option<String>,
}

#[derive(Default)]
pub(crate) struct Serials {
    by_id: HashMap<TermId, u64>,
    next: u64,
}

pub(crate) struct Phone {
    pub terms: Terminals,
    pub server: Mutex<Server>,
    pub conns: Mutex<HashMap<ConnId, Conn>>,
    pub tasks: Mutex<tasks::TaskState>,
    pub worklist_undo: Mutex<worklist::Undos>,
    pub serials: Mutex<Serials>,
    pub dark: AtomicBool,
    me: Weak<Phone>,
    changes: Mutex<(u64, Vec<(u64, Listener)>)>,
}

pub(crate) fn phone_socket_path() -> std::path::PathBuf {
    xenon_term::runtime_dir().join("phone.sock")
}

pub(crate) fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn run() -> io::Result<()> {
    let listener = control::listener()?;
    let env = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    let terms = Terminals::start(env);
    let phone = Phone::start(&terms);
    control::serve(&phone, &listener)
}

impl Phone {
    fn start(terms: &Terminals) -> Arc<Self> {
        let phone = Arc::new_cyclic(|me| Self {
            me: me.clone(),
            terms: terms.clone(),
            server: Mutex::new(Server::default()),
            conns: Mutex::new(HashMap::new()),
            tasks: Mutex::new(tasks::TaskState::default()),
            worklist_undo: Mutex::default(),
            serials: Mutex::default(),
            dark: AtomicBool::new(true),
            changes: Mutex::new((0, Vec::new())),
        });
        for info in terms.list() {
            phone.serial(info.id);
        }
        server::spawn_upkeep(&phone);
        phone.watch_events();
        phone.watch_settings();
        phone.apply_settings();
        phone
    }

    pub(crate) fn serial(&self, id: TermId) -> u64 {
        let mut serials = locked(&self.serials);
        if let Some(n) = serials.by_id.get(&id) {
            return *n;
        }
        serials.next += 1;
        let n = serials.next;
        serials.by_id.insert(id, n);
        n
    }

    fn watch_events(&self) {
        let rx = self.terms.events();
        let weak = self.me.clone();
        let spawned = std::thread::Builder::new()
            .name("xenon-phone-events".into())
            .spawn(move || {
                while let Ok(event) = rx.recv_blocking() {
                    let Some(phone) = weak.upgrade() else { break };
                    phone.on_event(event);
                }
            });
        if let Err(error) = spawned {
            log::error!("phone events thread: {error}");
        }
    }

    fn on_event(&self, event: Event) {
        match event {
            Event::Ready { id, .. } => {
                self.serial(id);
                self.push_dots();
            }
            Event::Status { id, status } => {
                self.push_dots();
                if status.exit.is_some() {
                    tasks::exited(self, id, status.exit);
                }
            }
            Event::Closed { id } => {
                self.push_dots();
                tasks::exited(self, id, None);
                self.terminal_closed(id);
            }
            Event::Failed { id, .. } => tasks::exited(self, id, Some(Exit::Code(127))),
            _ => {}
        }
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

    pub(crate) fn changed(&self) {
        for (_, listener) in locked(&self.changes).1.iter() {
            listener();
        }
    }

    fn terminal_closed(&self, id: TermId) {
        let conns: Vec<ConnId> = locked(&self.conns)
            .iter()
            .filter(|(_, c)| c.attach.as_ref().is_some_and(|a| a.term == id))
            .map(|(conn, _)| *conn)
            .collect();
        for conn in conns {
            self.detach(conn);
            self.closed_notice(conn);
        }
    }

    pub(crate) fn self_ref(&self) -> Weak<Phone> {
        self.me.clone()
    }

    pub(crate) fn send_to(&self, conn: ConnId, msg: crate::remote::ServerMsg) {
        if let Some(c) = locked(&self.conns).get(&conn) {
            c.out.try_send(msg);
        }
    }
}
