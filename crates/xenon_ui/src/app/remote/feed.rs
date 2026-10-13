//! A phone's fast path to one terminal, off the GPUI thread.
//!
//! Input: the connection thread writes typed text and named keys straight to
//! the PTY (`PhoneInput`). Output: the PTY thread pokes a worker on every
//! output batch; the worker snapshots the grid under the terminal lock,
//! diffs, and hands the frame to the connection's outbox. The UI thread only
//! does Mac-side bookkeeping and the occasional `sync` for resize/scroll.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use xenon_remote::{ClientMsg, FrameEncoder, InputSink, Outbox};
use xenon_terminal::RemoteIo;

use super::wire::wire_screen;

/// Frame coalescing window in low-bandwidth mode (≤ 30 fps while output streams).
const FLUSH_AFTER: Duration = Duration::from_millis(33);
/// Retry after a full outbox (slow phone link).
const RETRY_AFTER: Duration = Duration::from_millis(250);

static NEXT_HOOK: AtomicU64 = AtomicU64::new(1);

struct FeedState {
    io: RemoteIo,
    encoder: Mutex<FrameEncoder>,
    out: Outbox,
    low_bandwidth: AtomicBool,
}

impl FeedState {
    /// Snapshot, diff, and queue what changed. False when the outbox was full.
    fn emit(&self, woke: Instant) -> bool {
        let Ok(mut encoder) = self.encoder.lock() else {
            return true;
        };
        let started = Instant::now();
        let screen = wire_screen(&self.io.snapshot());
        let snapshot = started.elapsed();
        let delivered = encoder
            .encode(&screen)
            .into_iter()
            .all(|m| self.out.try_send(m));
        if !delivered {
            encoder.force_full();
        }
        log::debug!(
            "xenon remote: output wake->outbox {:?} (snapshot {snapshot:?}) delivered={delivered}",
            woke.elapsed()
        );
        delivered
    }
}

pub(super) struct Feed {
    state: Arc<FeedState>,
    hook_id: u64,
    poke: SyncSender<Instant>,
}

impl Feed {
    pub(super) fn new(io: RemoteIo, out: Outbox, low_bandwidth: bool) -> Self {
        let state = Arc::new(FeedState {
            io,
            encoder: Mutex::new(FrameEncoder::default()),
            out,
            low_bandwidth: AtomicBool::new(low_bandwidth),
        });
        let (poke, wakes) = sync_channel::<Instant>(1);
        let hook_id = NEXT_HOOK.fetch_add(1, Ordering::Relaxed);
        let from_pty = poke.clone();
        state.io.set_wake_hook(
            hook_id,
            Some(Arc::new(move || {
                let _ = from_pty.try_send(Instant::now());
            })),
        );
        let worker = state.clone();
        let spawned = std::thread::Builder::new()
            .name("xenon-remote-feed".into())
            .spawn(move || run(&worker, &wakes));
        if let Err(e) = spawned {
            log::warn!("xenon remote: feed thread: {e}");
        }
        Self {
            state,
            hook_id,
            poke,
        }
    }

    pub(super) fn io(&self) -> &RemoteIo {
        &self.state.io
    }

    pub(super) fn input_sink(&self) -> Arc<dyn InputSink> {
        Arc::new(PhoneInput(self.state.io.clone()))
    }

    /// Ask the worker for a frame (a no-op diff when nothing changed).
    pub(super) fn poke(&self) {
        let _ = self.poke.try_send(Instant::now());
    }

    pub(super) fn set_low_bandwidth(&self, on: bool) {
        self.state.low_bandwidth.store(on, Ordering::Relaxed);
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        // The hook held the other sender; with both gone the worker exits.
        self.state.io.set_wake_hook(self.hook_id, None);
    }
}

fn run(state: &FeedState, wakes: &std::sync::mpsc::Receiver<Instant>) {
    let mut retry = false;
    loop {
        let woke = if retry {
            match wakes.recv_timeout(RETRY_AFTER) {
                Ok(at) => at,
                Err(RecvTimeoutError::Timeout) => Instant::now(),
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match wakes.recv() {
                Ok(at) => at,
                Err(_) => return,
            }
        };
        if state.low_bandwidth.load(Ordering::Relaxed) {
            std::thread::sleep(FLUSH_AFTER);
        }
        while wakes.try_recv().is_ok() {}
        retry = !state.emit(woke);
    }
}

struct PhoneInput(RemoteIo);

impl InputSink for PhoneInput {
    fn send(&self, msg: &ClientMsg) -> bool {
        match msg {
            ClientMsg::Text { data } => {
                self.0.send_text(data);
                true
            }
            ClientMsg::Key { key } => self.0.send_named_key(key.keystroke()),
            _ => false,
        }
    }
}
