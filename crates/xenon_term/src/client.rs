//! The window's side of the socket.

use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Result, anyhow, bail};

use crate::wire::{self, Message, Request};
use crate::{Exit, Size, Spawn, TermId};

/// What the service tells the window.
#[derive(Debug)]
pub enum Event {
    /// Bytes to feed the view. The first one after `attach` is the replay.
    Output {
        id: TermId,
        bytes: Vec<u8>,
    },
    Exited {
        id: TermId,
        exit: Exit,
    },
    /// The socket closed.
    Lost,
    // Not implemented in the spike: Status, Size and Created.
}

#[derive(Debug)]
pub struct TermInfo {
    pub id: TermId,
    pub pid: i32,
    pub exit: Option<Exit>,
}

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Message>>>>;

pub struct Client {
    out: Mutex<UnixStream>,
    next: AtomicU64,
    pending: Pending,
    events: async_channel::Receiver<Event>,
}

impl Client {
    /// Connect to a running service and send this window's environment (`KEY=VALUE`),
    /// the base for every spawn.
    ///
    /// Not implemented in the spike: starting the service through socket activation.
    pub fn connect(path: &Path, env: Vec<String>) -> Result<Self> {
        let stream = UnixStream::connect(path)
            .map_err(|e| anyhow!("no terminal service at {}: {e}", path.display()))?;
        let mut input = stream.try_clone()?;
        let pending: Pending = Arc::default();
        let (events_tx, events) = async_channel::unbounded();
        let table = Arc::clone(&pending);
        thread::spawn(move || {
            while let Ok(msg) = wire::read_message(&mut input) {
                route(&table, &events_tx, msg);
            }
            let _ = events_tx.send_blocking(Event::Lost);
            table.lock().unwrap_or_else(|e| e.into_inner()).clear();
        });
        let client = Self {
            out: Mutex::new(stream),
            next: AtomicU64::new(1),
            pending,
            events,
        };
        match client.call(Request::Hello {
            version: wire::VERSION,
            env,
        })? {
            Message::Ok(_) => Ok(client),
            Message::Err(_, e) => bail!(e),
            _ => bail!("unexpected reply"),
        }
    }

    pub fn events(&self) -> async_channel::Receiver<Event> {
        self.events.clone()
    }

    pub fn list(&self) -> Result<Vec<TermInfo>> {
        match self.call(Request::List)? {
            Message::Listed(_, items) => Ok(items
                .into_iter()
                .map(|i| TermInfo {
                    id: i.id,
                    pid: i.pid,
                    exit: i.exit,
                })
                .collect()),
            Message::Err(_, e) => bail!(e),
            _ => bail!("unexpected reply"),
        }
    }

    pub fn create(&self, spawn: Spawn, size: Size) -> Result<TermId> {
        let req = Request::Create {
            cwd: spawn.cwd.to_string_lossy().into_owned(),
            argv: spawn.argv,
            env: spawn.env,
            size,
        };
        match self.call(req)? {
            Message::Created(_, id) => Ok(id),
            Message::Err(_, e) => bail!(e),
            _ => bail!("unexpected reply"),
        }
    }

    /// Resize to `size`, then send the replay as the first `Event::Output`, then live
    /// output. Attaching again replays again.
    pub fn attach(&self, id: TermId, size: Size) -> Result<()> {
        self.send(0, &Request::Attach { id, size })
    }

    pub(crate) fn send(&self, seq: u64, req: &Request) -> Result<()> {
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        wire::write_request(&mut *out, seq, req).map_err(|e| anyhow!("terminal service gone: {e}"))
    }

    fn call(&self, req: Request) -> Result<Message> {
        let seq = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(seq, tx);
        self.send(seq, &req)?;
        rx.recv().map_err(|_| anyhow!("terminal service gone"))
    }
}

fn route(table: &Pending, events: &async_channel::Sender<Event>, msg: Message) {
    let seq = match &msg {
        Message::Ok(s) | Message::Err(s, _) | Message::Created(s, _) | Message::Listed(s, _) => *s,
        Message::Output { id, bytes } => {
            let _ = events.send_blocking(Event::Output {
                id: *id,
                bytes: bytes.clone(),
            });
            return;
        }
        Message::Exited { id, exit } => {
            let _ = events.send_blocking(Event::Exited {
                id: *id,
                exit: *exit,
            });
            return;
        }
    };
    if let Some(tx) = table.lock().unwrap_or_else(|e| e.into_inner()).remove(&seq) {
        let _ = tx.send(msg);
    }
}
