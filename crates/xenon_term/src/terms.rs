//! `Terms`: the service core. It holds every terminal and is the one API behind both
//! the window's socket and the phone server.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use xenon_core::{TermId, WorkspaceId};

use crate::conn::Link;
use crate::hub::{Hub, Note};
use crate::reader;
use crate::term::Term;
use crate::{Claim, Size, Spawn, TermInfo, pty};

/// Size of a terminal nobody has claimed yet.
const FIRST_SIZE: Size = Size { cols: 80, rows: 24 };

pub(crate) struct Terms {
    /// The last window's environment; only a window connecting writes it.
    base_env: Mutex<Vec<String>>,
    terms: Mutex<HashMap<TermId, Arc<Term>>>,
    pub hub: Arc<Hub>,
    /// Whether the window's theme is dark: the phone follows it by default.
    pub dark: AtomicBool,
}

/// What the service itself was started with, until a window sends its own.
fn service_env() -> Vec<String> {
    std::env::vars()
        .filter(|(k, _)| !k.starts_with("LISTEN_") && k != "NOTIFY_SOCKET")
        .map(|(k, v)| format!("{k}={v}"))
        .collect()
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Terms {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            base_env: Mutex::new(service_env()),
            terms: Mutex::new(HashMap::new()),
            hub: Arc::new(Hub::default()),
            dark: AtomicBool::new(true),
        })
    }

    pub(crate) fn set_env(&self, env: Vec<String>) {
        *locked(&self.base_env) = env;
    }

    pub(crate) fn get(&self, id: TermId) -> Option<Arc<Term>> {
        locked(&self.terms).get(&id).cloned()
    }

    pub(crate) fn all(&self) -> Vec<Arc<Term>> {
        locked(&self.terms).values().cloned().collect()
    }

    pub(crate) fn list(&self) -> Vec<TermInfo> {
        self.all().iter().map(|t| t.info()).collect()
    }

    pub(crate) fn create(
        self: &Arc<Self>,
        workspace: WorkspaceId,
        spawn: &Spawn,
        size: Option<Size>,
    ) -> io::Result<TermId> {
        let mut vars: HashMap<String, String> = HashMap::new();
        let extra = [
            format!("XENON_WORKSPACE={workspace}"),
            "TERM=xterm-256color".into(),
            "COLORTERM=truecolor".into(),
        ];
        for pair in locked(&self.base_env)
            .iter()
            .chain(&spawn.env)
            .chain(extra.iter())
        {
            if let Some((k, v)) = pair.split_once('=') {
                vars.insert(k.to_owned(), v.to_owned());
            }
        }
        let size = size.unwrap_or(FIRST_SIZE);
        let live = pty::spawn(&spawn.cwd, &spawn.argv, &vars, size)?;
        let pid = live.child.id() as i32;
        let master = live.master.try_clone()?;
        let cwd = Path::new(&spawn.cwd).to_owned();
        let term = Arc::new(Term::new(workspace, pid, cwd, master, size));
        let id = term.id;
        locked(&self.terms).insert(id, Arc::clone(&term));
        let hub = Arc::clone(&self.hub);
        thread::Builder::new()
            .name(format!("xenon-term-{}", term.serial))
            .spawn(move || {
                // The reader owns the child: it reaps it when the terminal ends.
                if let Err(error) = reader::run(&term, live, &hub) {
                    log::warn!("terminal {}: {error}", term.serial);
                }
            })?;
        self.hub.emit(&[Note::Born { id, workspace }]);
        Ok(id)
    }

    pub(crate) fn attach(&self, id: TermId, link: Link, size: Size) -> bool {
        let Some(term) = self.get(id) else {
            return false;
        };
        self.hub.emit(&term.attach(link, size));
        true
    }

    pub(crate) fn claim(&self, id: TermId, conn: u64, claim: Claim) {
        if let Some(term) = self.get(id) {
            self.hub.emit(&term.claim(conn, claim));
        }
    }

    /// `desk` is true for the window's typing, false for a phone's.
    pub(crate) fn input(&self, id: TermId, bytes: &[u8], desk: bool) {
        if let Some(term) = self.get(id) {
            self.hub.emit(&term.input(bytes, desk));
        }
    }

    pub(crate) fn seen(&self, id: TermId) {
        if let Some(term) = self.get(id) {
            self.hub.emit(&term.seen());
        }
    }

    /// Hang up the child's session and forget the terminal. Idempotent.
    pub(crate) fn close(&self, id: TermId) {
        let Some(term) = locked(&self.terms).remove(&id) else {
            return;
        };
        // An exited child is reaped, and its pid may belong to someone else by now.
        if term.info().status.exit.is_none() {
            term.mark_closed();
            pty::hangup(term.pid);
        }
        self.hub.emit(&[Note::Closed { id }]);
    }

    /// A connection went away: it is no longer a desk, and its claims lapse.
    pub(crate) fn detach_conn(&self, conn: u64) {
        for term in self.all() {
            self.hub.emit(&term.detach(conn));
        }
    }
}
