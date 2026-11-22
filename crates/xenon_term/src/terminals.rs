use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use xenon_core::{TermId, WorkspaceId};

use crate::holder_link::{self, HolderLink};
use crate::links::{Call, Links};
use crate::start::{self, Spec};
use crate::{Size, Spawn, TermInfo, TermStatus, builds, dir};

const CLOSE_PATIENCE: Duration = Duration::from_secs(8);

const UNIT_VARS: [&str; 11] = [
    "LISTEN_PID",
    "LISTEN_FDS",
    "LISTEN_FDNAMES",
    "NOTIFY_SOCKET",
    "INVOCATION_ID",
    "JOURNAL_STREAM",
    "MANAGERPID",
    "MANAGERPIDFDID",
    "SYSTEMD_EXEC_PID",
    "MEMORY_PRESSURE_WATCH",
    "MEMORY_PRESSURE_WRITE",
];

#[derive(Debug)]
pub enum Event {
    /// A holder is connected (window- or phone-made, or found at start): adopt it.
    Ready {
        id: TermId,
        info: TermInfo,
    },
    /// Bytes to feed the view. The first one after `attach` is the replay.
    Output {
        id: TermId,
        bytes: Vec<u8>,
    },
    Status {
        id: TermId,
        status: TermStatus,
    },
    /// The size in force changed: make the view this size.
    Size {
        id: TermId,
        size: Size,
    },
    Closed {
        id: TermId,
    },
    /// Only systemd refusing to start a holder reaches here.
    Failed {
        id: TermId,
        error: String,
    },
    Builds,
}
pub(crate) struct Shared {
    base_env: Vec<String>,
    pub(crate) links: Links,
    tx: async_channel::Sender<Event>,
    rx: async_channel::Receiver<Event>,
}

impl Shared {
    pub(crate) fn emit(&self, event: Event) {
        let _ = self.tx.send_blocking(event);
    }

    pub(crate) fn remove(&self, link: &Arc<HolderLink>) {
        if self.links.remove(link) {
            self.emit(Event::Closed { id: link.id });
        }
    }

    fn scan(self: &Arc<Self>) {
        let fresh: Vec<TermId> = dir::ids()
            .into_iter()
            .filter(|id| self.links.expect(*id))
            .collect();
        thread::scope(|scope| {
            for id in fresh {
                scope.spawn(move || {
                    if let Err(error) = self.adopt(id) {
                        log::debug!("terminal {id}: {error}");
                    }
                });
            }
        });
    }

    pub(crate) fn adopt(self: &Arc<Self>, id: TermId) -> std::io::Result<()> {
        if !self.links.begin_connect(id) {
            return Ok(());
        }
        let result = holder_link::adopt(self, id);
        if result.is_err() {
            self.links.connect_failed(id);
        }
        result
    }
}

#[derive(Clone)]
pub struct Terminals {
    pub(crate) shared: Arc<Shared>,
}

impl Terminals {
    /// Find every holder in the runtime directory, connect to each, and keep watching
    /// the directory and the installed binary. Terminals opened here start from
    /// `base_env` (`KEY=VALUE`), less what systemd set for this process alone.
    pub fn start(base_env: Vec<String>) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let base_env = base_env
            .into_iter()
            .filter(|pair| {
                let key = pair.split_once('=').map_or(pair.as_str(), |(k, _)| k);
                !UNIT_VARS.contains(&key)
            })
            .collect();
        let shared = Arc::new(Shared {
            base_env,
            links: Links::default(),
            tx,
            rx,
        });
        let dir = dir::term_dir();
        if let Err(error) = std::fs::create_dir_all(&dir) {
            log::warn!("runtime directory {}: {error}", dir.display());
        }
        shared.scan();
        let watching = Arc::clone(&shared);
        if let Err(error) = dir::watch_dir(&dir, move || watching.scan()) {
            log::warn!("watch {}: {error}", dir.display());
        }
        builds::watch(&shared);
        Self { shared }
    }

    pub fn events(&self) -> async_channel::Receiver<Event> {
        self.shared.rx.clone()
    }

    pub fn list(&self) -> Vec<TermInfo> {
        let mut list: Vec<TermInfo> = self.shared.links.live().iter().map(|l| l.info()).collect();
        list.sort_by_key(|t| t.pid);
        list
    }

    pub fn info(&self, id: TermId) -> Option<TermInfo> {
        self.shared.links.get(id).map(|link| link.info())
    }

    /// Start a terminal. The id is chosen here, before the holder exists, so the tab and
    /// the session file can record it at once; `Event::Ready` follows when it runs.
    pub fn open(&self, workspace: WorkspaceId, spawn: Spawn) -> TermId {
        let id = TermId::new();
        self.shared.links.expect(id);
        let shared = Arc::clone(&self.shared);
        thread::spawn(move || {
            let mut env = shared.base_env.clone();
            env.extend(spawn.env);
            let spec = Spec {
                v: 1,
                workspace: Some(workspace),
                cwd: spawn.cwd.to_string_lossy().into_owned(),
                argv: spawn.argv,
                env,
            };
            let result = start::launch(id, &spec).and_then(|()| shared.adopt(id));
            if let Err(error) = result {
                shared.links.connect_failed(id);
                shared.emit(Event::Failed {
                    id,
                    error: error.to_string(),
                });
            }
        });
        id
    }

    /// Block until the terminal is connected, or `patience` runs out. For callers that
    /// need it at once, such as a phone that opens a terminal and attaches to it.
    pub fn wait_ready(&self, id: TermId, patience: Duration) -> bool {
        let until = Instant::now() + patience;
        while self.shared.links.get(id).is_none() {
            if Instant::now() >= until {
                return false;
            }
            thread::sleep(Duration::from_millis(15));
        }
        true
    }

    /// Close one terminal: the holder hangs up its session, then exits. Idempotent.
    pub fn close(&self, id: TermId) {
        if !self.shared.links.send(id, Call::Close) {
            return;
        }
        let shared = Arc::clone(&self.shared);
        thread::spawn(move || {
            thread::sleep(CLOSE_PATIENCE);
            if shared.links.get(id).is_some() {
                stop_unit(id);
            }
        });
    }
}

fn stop_unit(id: TermId) {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "stop", &start::unit_name(id)])
        .output();
}
