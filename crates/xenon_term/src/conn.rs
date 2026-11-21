//! A window's connection: the queue to its writer thread and how much output is stuck
//! in it.

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use xenon_core::TermId;

use crate::wire::{self, Message};

/// A window that falls more than this far behind is dropped and repainted when it
/// catches up.
pub(crate) const SLOW_LIMIT: usize = 4 << 20;

#[derive(Clone)]
pub(crate) struct Link {
    pub conn: u64,
    tx: Sender<Message>,
    queued: Arc<AtomicUsize>,
}

impl Link {
    /// A link and the writer thread that drains it into `out`.
    pub(crate) fn start(conn: u64, mut out: impl Write + Send + 'static) -> Self {
        let (tx, rx) = channel::<Message>();
        let queued = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&queued);
        thread::spawn(move || write_loop(&rx, &mut out, &counted));
        Self { conn, tx, queued }
    }

    /// False when the connection is gone.
    pub(crate) fn send(&self, msg: Message) -> bool {
        if let Message::Output { bytes, .. } = &msg {
            self.queued.fetch_add(bytes.len(), Ordering::Relaxed);
        }
        self.tx.send(msg).is_ok()
    }

    pub(crate) fn output(&self, id: TermId, bytes: Vec<u8>) -> bool {
        self.send(Message::Output { id, bytes })
    }

    /// Output bytes accepted but not yet written to the socket.
    pub(crate) fn queued(&self) -> usize {
        self.queued.load(Ordering::Relaxed)
    }
}

fn write_loop(rx: &Receiver<Message>, out: &mut impl Write, queued: &AtomicUsize) {
    while let Ok(msg) = rx.recv() {
        let size = match &msg {
            Message::Output { bytes, .. } => bytes.len(),
            _ => 0,
        };
        let result: io::Result<()> = wire::write_message(out, &msg);
        queued.fetch_sub(size, Ordering::Relaxed);
        if result.is_err() {
            break;
        }
    }
}
