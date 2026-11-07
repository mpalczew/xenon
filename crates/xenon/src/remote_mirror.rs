//! Mirror one VTE terminal to a phone.
//!
//! Costs nothing until a phone attaches: the `contents-changed` handler exists
//! only while one is watching. Each burst of changes is coalesced into at most
//! one dump per `GAP` (about 30 a second); the dump is VTE's visible screen as
//! HTML, which a worker thread parses, diffs against what the phone has, and
//! queues. The GTK thread does only the dump, so a slow phone link or a big
//! screen never holds up typing.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;
use vte::prelude::*;
use xenon_remote::{CursorShapeWire, CursorWire, FrameEncoder, Outbox, Rgb, VteSnapshot};

/// Shortest time between dumps; the phone sees at most ~30 frames a second.
const GAP: Duration = Duration::from_millis(33);
/// Same, when the phone asked for low bandwidth.
const LOW_GAP: Duration = Duration::from_millis(100);
/// Try again after a full outbox (slow phone link).
const RETRY_AFTER: Duration = Duration::from_millis(250);

enum Job {
    Frame(VteSnapshot),
    /// Send everything again (new colors, new size).
    Full,
}

struct Shared {
    term: glib::WeakRef<vte::Terminal>,
    pending: Cell<bool>,
    last: Cell<Instant>,
    gap: Cell<Duration>,
    /// Default foreground and background the phone page uses.
    colors: Cell<(Rgb, Rgb)>,
    jobs: Sender<Job>,
}

pub(crate) struct Mirror {
    shared: Rc<Shared>,
    term: vte::Terminal,
    handler: Option<glib::SignalHandlerId>,
    /// Scrolling moves the viewport without changing the contents.
    scroll: Option<(gtk::Adjustment, glib::SignalHandlerId)>,
}

impl Mirror {
    pub(crate) fn start(
        term: &vte::Terminal,
        out: Outbox,
        colors: (Rgb, Rgb),
        low_bandwidth: bool,
    ) -> Self {
        let (jobs, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("xenon-remote-mirror".into())
            .spawn(move || run(&rx, &out));
        if let Err(error) = spawned {
            eprintln!("[xg] phone mirror thread: {error}");
        }
        let shared = Rc::new(Shared {
            term: term.downgrade(),
            pending: Cell::new(false),
            last: Cell::new(Instant::now() - GAP),
            gap: Cell::new(if low_bandwidth { LOW_GAP } else { GAP }),
            colors: Cell::new(colors),
            jobs,
        });
        let watcher = shared.clone();
        let handler = term.connect_contents_changed(move |_| watcher.poke());
        let watcher = shared.clone();
        let scroll = term.vadjustment().map(|adj| {
            (
                adj.clone(),
                adj.connect_value_changed(move |_| watcher.poke()),
            )
        });
        shared.poke();
        Self {
            shared,
            term: term.clone(),
            handler: Some(handler),
            scroll,
        }
    }

    /// Dump the screen soon (after a resize, say).
    pub(crate) fn poke(&self) {
        self.shared.poke();
    }

    pub(crate) fn set_low_bandwidth(&self, on: bool) {
        self.shared.gap.set(if on { LOW_GAP } else { GAP });
    }

    /// New default colors: resend every row.
    pub(crate) fn set_colors(&self, colors: (Rgb, Rgb)) {
        self.shared.colors.set(colors);
        let _ = self.shared.jobs.send(Job::Full);
        self.shared.poke();
    }
}

impl Drop for Mirror {
    fn drop(&mut self) {
        if let Some(id) = self.handler.take() {
            self.term.disconnect(id);
        }
        if let Some((adj, id)) = self.scroll.take() {
            adj.disconnect(id);
        }
    }
}

impl Shared {
    /// One dump per quiet gap, however many change signals arrive.
    fn poke(self: &Rc<Self>) {
        if self.pending.replace(true) {
            return;
        }
        let wait = self.gap.get().saturating_sub(self.last.get().elapsed());
        let this = self.clone();
        glib::timeout_add_local_once(wait, move || {
            this.pending.set(false);
            this.last.set(Instant::now());
            if let Some(term) = this.term.upgrade() {
                let _ = this
                    .jobs
                    .send(Job::Frame(capture(&term, this.colors.get())));
            }
        });
    }
}

/// The visible screen, its size, the cursor in viewport rows, and whether the
/// viewport is scrolled back.
fn capture(term: &vte::Terminal, (default_fg, default_bg): (Rgb, Rgb)) -> VteSnapshot {
    let html = term
        .text_format(vte::Format::Html)
        .map(|s| s.to_string())
        .unwrap_or_default();
    let cols = term.column_count().clamp(1, i64::from(u16::MAX)) as u16;
    let rows = term.row_count().clamp(1, i64::from(u16::MAX)) as u16;
    let (col, row) = term.cursor_position();
    let unit = term.char_height().max(1) as f64;
    let (top, scrolled) = match term.vadjustment() {
        Some(adj) => (
            (adj.value() / unit).round() as i64,
            adj.value() + unit / 2.0 < adj.upper() - adj.page_size(),
        ),
        None => (0, false),
    };
    let view_row = row - top;
    let cursor = (0..i64::from(rows))
        .contains(&view_row)
        .then(|| CursorWire {
            r: view_row as u16,
            c: col.clamp(0, i64::from(cols) - 1) as u16,
            s: match term.cursor_shape() {
                vte::CursorShape::Ibeam => CursorShapeWire::Bar,
                vte::CursorShape::Underline => CursorShapeWire::Underline,
                _ => CursorShapeWire::Block,
            },
        })
        .filter(|_| !scrolled);
    VteSnapshot {
        html,
        cols,
        rows,
        cursor,
        scrolled,
        default_fg,
        default_bg,
    }
}

/// Worker: parse, diff, and queue. Blocks while nothing changes.
fn run(jobs: &Receiver<Job>, out: &Outbox) {
    let mut encoder = FrameEncoder::default();
    let mut last: Option<VteSnapshot> = None;
    let mut retry = false;
    loop {
        let first = if retry {
            match jobs.recv_timeout(RETRY_AFTER) {
                Ok(job) => Some(job),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match jobs.recv() {
                Ok(job) => Some(job),
                Err(_) => return,
            }
        };
        let mut full = false;
        for job in first.into_iter().chain(jobs.try_iter()) {
            match job {
                Job::Frame(snapshot) => last = Some(snapshot),
                Job::Full => full = true,
            }
        }
        if full {
            encoder.force_full();
        }
        let Some(snapshot) = last.clone() else {
            continue;
        };
        let delivered = encoder
            .encode(&snapshot.into_screen())
            .into_iter()
            .all(|m| out.try_send(m));
        if !delivered {
            encoder.force_full();
        }
        retry = !delivered;
    }
}
