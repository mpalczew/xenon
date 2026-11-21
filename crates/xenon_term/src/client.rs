//! The window's side of the socket.

use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Result, anyhow, bail};
use xenon_core::{TermId, WorkspaceId};

use crate::phone_link::PhoneLink;
use crate::wire::{self, Message, Request};
use crate::{Size, Spawn, TermInfo, TermStatus};

/// What the service tells the window.
#[derive(Debug)]
pub enum Event {
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
    /// A terminal was made, perhaps from the phone: the window adopts it.
    Created {
        id: TermId,
        workspace: WorkspaceId,
    },
    /// A terminal was closed, perhaps from the phone.
    Closed {
        id: TermId,
    },
    /// The phone server's state changed: ask `phone().status()`.
    Phone,
    /// The socket closed.
    Lost,
}

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Message>>>>;

pub struct Client {
    out: Mutex<UnixStream>,
    next: AtomicU64,
    pending: Pending,
    events: async_channel::Receiver<Event>,
}

impl Client {
    /// Connect to the service, which systemd starts on demand, and send this window's
    /// environment (`KEY=VALUE`) as the base for every spawn.
    pub fn connect(env: Vec<String>) -> Result<Self> {
        let path = crate::socket_path();
        let stream = UnixStream::connect(&path).map_err(|e| {
            anyhow!(
                "no terminal service at {} ({e}); `scripts/release/install` sets up its systemd socket",
                path.display()
            )
        })?;
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
        client.ok(Request::Hello {
            version: wire::VERSION,
            env,
        })?;
        Ok(client)
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
                    workspace: i.workspace,
                    pid: i.pid,
                    status: i.status,
                })
                .collect()),
            Message::Err(_, e) => bail!(e),
            _ => bail!("unexpected reply"),
        }
    }

    pub fn create(&self, workspace: WorkspaceId, spawn: Spawn) -> Result<TermId> {
        let req = Request::Create {
            workspace,
            cwd: spawn.cwd.to_string_lossy().into_owned(),
            argv: spawn.argv,
            env: spawn.env,
        };
        match self.call(req)? {
            Message::Created(_, id) => Ok(id),
            Message::Err(_, e) => bail!(e),
            _ => bail!("unexpected reply"),
        }
    }

    pub fn phone(&self) -> PhoneLink<'_> {
        PhoneLink(self)
    }

    pub(crate) fn send(&self, seq: u64, req: &Request) {
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        // A broken socket also ends the reader thread, which reports `Event::Lost`.
        let _ = wire::write_request(&mut *out, seq, req);
    }

    pub(crate) fn call(&self, req: Request) -> Result<Message> {
        let seq = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(seq, tx);
        self.send(seq, &req);
        rx.recv().map_err(|_| anyhow!("terminal service gone"))
    }

    fn ok(&self, req: Request) -> Result<()> {
        match self.call(req)? {
            Message::Ok(_) => Ok(()),
            Message::Err(_, e) => bail!(e),
            _ => bail!("unexpected reply"),
        }
    }
}

fn route(table: &Pending, events: &async_channel::Sender<Event>, msg: Message) {
    let event = match msg {
        Message::Output { id, bytes } => Event::Output { id, bytes },
        Message::Status { id, status } => Event::Status { id, status },
        Message::Size { id, size } => Event::Size { id, size },
        Message::Born { id, workspace } => Event::Created { id, workspace },
        Message::Closed { id } => Event::Closed { id },
        Message::PhoneChanged => Event::Phone,
        Message::Ok(seq)
        | Message::Err(seq, _)
        | Message::Created(seq, _)
        | Message::Listed(seq, _)
        | Message::Reply(seq, _) => {
            let waiting = table.lock().unwrap_or_else(|e| e.into_inner()).remove(&seq);
            if let Some(tx) = waiting {
                let _ = tx.send(msg);
            }
            return;
        }
    };
    let _ = events.send_blocking(event);
}
