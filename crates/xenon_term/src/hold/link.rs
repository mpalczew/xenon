use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use serde::Serialize;

use crate::proto;

pub(crate) const SLOW_LIMIT: usize = 4 << 20;

enum Out {
    Control(Vec<u8>),
    Data(Vec<u8>),
}

#[derive(Clone)]
pub(crate) struct Link {
    pub conn: u64,
    tx: Sender<Out>,
    queued: Arc<AtomicUsize>,
}

impl Link {
    pub(crate) fn start(conn: u64, mut out: impl Write + Send + 'static) -> Self {
        let (tx, rx) = channel::<Out>();
        let queued = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&queued);
        thread::spawn(move || write_loop(&rx, &mut out, &counted));
        Self { conn, tx, queued }
    }

    pub(crate) fn send(&self, msg: &impl Serialize) -> bool {
        self.tx.send(Out::Control(proto::encode(msg))).is_ok()
    }

    pub(crate) fn output(&self, bytes: Vec<u8>) -> bool {
        self.queued.fetch_add(bytes.len(), Ordering::Relaxed);
        self.tx.send(Out::Data(bytes)).is_ok()
    }

    pub(crate) fn queued(&self) -> usize {
        self.queued.load(Ordering::Relaxed)
    }
}

fn write_loop(rx: &Receiver<Out>, out: &mut impl Write, queued: &AtomicUsize) {
    while let Ok(msg) = rx.recv() {
        let (result, size) = match &msg {
            Out::Control(body) => (proto::write_raw_control(out, body), 0),
            Out::Data(bytes) => (proto::write_data(out, bytes), bytes.len()),
        };
        queued.fetch_sub(size, Ordering::Relaxed);
        if result.is_err() {
            break;
        }
    }
}
