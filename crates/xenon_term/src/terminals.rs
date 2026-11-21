//! `Terminals`: the window's and the phone server's view of every terminal. There is no
//! connect step that can fail: the runtime directory is the list, and one connection to
//! each holder found there is the only state.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use xenon_core::{TermId, WorkspaceId};

use crate::holder_link::{self, HolderLink};
use crate::proto::Frozen;
use crate::start::{self, Spec};
use crate::{Size, Spawn, TermInfo, TermStatus, builds, dir};

/// A holder that has not gone away this long after `Close` is stopped through systemd.
const CLOSE_PATIENCE: Duration = Duration::from_secs(8);

/// Who is asking. The window is the desk; the phone server watches through mirrors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Desk,
    Phone,
}

/// What holders tell the window.
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
    /// The holder is gone, for any reason.
    Closed {
        id: TermId,
    },
    /// Only systemd refusing to start a holder reaches here.
    Failed {
        id: TermId,
        error: String,
    },
    /// An install replaced the binary: marks need repainting.
    Builds,
}

/// Attach, size and typing that arrived while `open` was still starting the holder.
#[derive(Default)]
pub(crate) struct Early {
    pub size: Option<Size>,
    pub attach: bool,
    pub input: Vec<u8>,
}

pub(crate) struct Shared {
    pub role: Role,
    holders: Mutex<HashMap<TermId, Arc<HolderLink>>>,
    connecting: Mutex<HashSet<TermId>>,
    /// What the window did to a terminal before its holder was connected.
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

    /// Add a connected holder after replaying what the window did to it before. Under the same
    /// lock that `early` callers hold while they look for the link, so nothing is lost
    /// between "no link yet" and "remember it".
    pub(crate) fn insert(&self, link: Arc<HolderLink>, replay: impl FnOnce(Early)) {
        let mut early = locked(&self.early);
        if let Some(taken) = early.remove(&link.id) {
            // Before the link is visible, so later calls cannot overtake it.
            replay(taken);
        }
        locked(&self.holders).insert(link.id, link);
    }

    /// Send to the holder when it is connected, else remember it for when it is.
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

    /// The connection ended: the terminal is gone.
    pub(crate) fn remove(&self, link: &Arc<HolderLink>) {
        let mut holders = locked(&self.holders);
        if holders.get(&link.id).is_some_and(|l| Arc::ptr_eq(l, link)) {
            holders.remove(&link.id);
            drop(holders);
            self.emit(Event::Closed { id: link.id });
        }
    }

    /// Connect to every socket in the directory that we do not know yet.
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

    /// Connect once, however many callers race (the directory watch and `open`).
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
    /// the directory and the installed binary.
    pub fn start(role: Role) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let shared = Arc::new(Shared {
            role,
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

    /// Every terminal: no IO but a few stats.
    pub fn list(&self) -> Vec<TermInfo> {
        let links: Vec<_> = locked(&self.shared.holders).values().cloned().collect();
        let mut list: Vec<TermInfo> = links.iter().map(|l| l.info()).collect();
        list.sort_by_key(|t| t.pid);
        list
    }

    /// Start a terminal. The id is chosen here, before the holder exists, so the tab and
    /// the session file can record it at once; `Event::Ready` follows when it runs.
    pub fn open(&self, workspace: WorkspaceId, spawn: Spawn) -> TermId {
        let id = TermId::new();
        let shared = Arc::clone(&self.shared);
        thread::spawn(move || {
            let mut env = start::base_env();
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

    /// The window's environment, for terminals the phone makes. Only the desk writes it.
    pub fn set_base_env(&self, env: Vec<String>) {
        if self.shared.role == Role::Desk
            && let Err(error) = start::write_base_env(&env)
        {
            log::warn!("write base env: {error}");
        }
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

/// A hung holder: ask systemd to stop its unit.
fn stop_unit(id: TermId) {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "stop", &start::unit_name(id)])
        .output();
}
