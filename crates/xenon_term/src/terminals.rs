use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use xenon_core::{TermId, WorkspaceId};

use crate::holder_link::{self, HolderLink};
use crate::proto::Frozen;
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

#[derive(Default)]
pub(crate) struct Early {
    pub size: Option<Size>,
    pub attach: bool,
    pub input: Vec<u8>,
}

pub(crate) struct Shared {
    base_env: Vec<String>,
    holders: Mutex<HashMap<TermId, Arc<HolderLink>>>,
    connecting: Mutex<HashSet<TermId>>,
    pub(crate) early: Mutex<HashMap<TermId, Early>>,
    tx: async_channel::Sender<Event>,
    rx: async_channel::Receiver<Event>,
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Shared {
    pub(crate) fn emit(&self, event: Event) {
        let _ = self.tx.send_blocking(event);
    }

    pub(crate) fn link(&self, id: TermId) -> Option<Arc<HolderLink>> {
        locked(&self.holders).get(&id).cloned()
    }

    pub(crate) fn insert(&self, link: Arc<HolderLink>) -> Option<Early> {
        let mut early = locked(&self.early);
        let taken = early.remove(&link.id);
        locked(&self.holders).insert(link.id, link);
        taken
    }

    pub(crate) fn to_link_or_early(
        &self,
        id: TermId,
        send: impl FnOnce(&HolderLink),
        remember: impl FnOnce(&mut Early),
    ) {
        let mut early = locked(&self.early);
        match self.link(id) {
            Some(link) => {
                drop(early);
                send(&link);
            }
            None => remember(early.entry(id).or_default()),
        }
    }

    pub(crate) fn remove(&self, link: &Arc<HolderLink>) {
        let mut holders = locked(&self.holders);
        if holders.get(&link.id).is_some_and(|l| Arc::ptr_eq(l, link)) {
            holders.remove(&link.id);
            drop(holders);
            self.emit(Event::Closed { id: link.id });
        }
    }

    fn scan(self: &Arc<Self>) {
        let fresh: Vec<TermId> = dir::ids()
            .into_iter()
            .filter(|id| locked(&self.holders).get(id).is_none())
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
        if self.link(id).is_some() || !locked(&self.connecting).insert(id) {
            return Ok(());
        }
        let result = holder_link::adopt(self, id);
        locked(&self.connecting).remove(&id);
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
            holders: Mutex::default(),
            connecting: Mutex::default(),
            early: Mutex::default(),
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
        let links: Vec<_> = locked(&self.shared.holders).values().cloned().collect();
        let mut list: Vec<TermInfo> = links.iter().map(|l| l.info()).collect();
        list.sort_by_key(|t| t.pid);
        list
    }

    pub fn info(&self, id: TermId) -> Option<TermInfo> {
        self.shared.link(id).map(|link| link.info())
    }

    /// Start a terminal. The id is chosen here, before the holder exists, so the tab and
    /// the session file can record it at once; `Event::Ready` follows when it runs.
    pub fn open(&self, workspace: WorkspaceId, spawn: Spawn) -> TermId {
        let id = TermId::new();
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
                locked(&shared.early).remove(&id);
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
        while self.shared.link(id).is_none() {
            if Instant::now() >= until {
                return false;
            }
            thread::sleep(Duration::from_millis(15));
        }
        true
    }

    /// Close one terminal: the holder hangs up its session, then exits. Idempotent.
    pub fn close(&self, id: TermId) {
        let Some(link) = self.shared.link(id) else {
            return;
        };
        link.send(&Frozen::Close);
        let shared = Arc::clone(&self.shared);
        thread::spawn(move || {
            thread::sleep(CLOSE_PATIENCE);
            if shared.link(id).is_some() {
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
