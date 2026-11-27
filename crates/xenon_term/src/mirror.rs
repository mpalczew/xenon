use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::TermMode;
use xenon_core::TermId;

use crate::hold::{Emu, FIRST_SIZE};
use crate::proto::v1::{FromHolder, ToHolder};
use crate::proto::{self, Control, Frame};
use crate::{Size, Terminals, dir};

/// The terminal is gone, or its holder speaks a protocol this build cannot read.
#[derive(Debug)]
pub struct Gone;

mod calls;
mod numbering;

pub use numbering::Numbering;
use numbering::{Cause, LineCount, Probe, SLACK, SLICE};

#[derive(Clone, Copy, Debug)]
pub struct Modes {
    pub size: Size,
    pub app_cursor: bool,
    pub mouse: bool,
    pub sgr_mouse: bool,
    pub alt: bool,
    pub alt_scroll: bool,
}

#[derive(Default)]
struct Flags {
    dirty: bool,
    stop: bool,
}

/// The emulator and its line numbering, behind one lock so a read can't tear.
struct Counted {
    emu: Emu,
    count: LineCount,
    tail: Vec<u8>,
    /// Scrollback the phone can reach. The emulator holds `SLACK` more, trimmed back
    /// after every slice, so growth within a slice is always measurable.
    cap: usize,
}

struct Inner {
    emu: Mutex<Counted>,
    flags: Mutex<Flags>,
    wake: Condvar,
    closed: AtomicBool,
}

pub struct Mirror {
    out: Mutex<std::os::unix::net::UnixStream>,
    inner: Arc<Inner>,
    label: String,
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Terminals {
    /// Watch a terminal on a connection of its own. `label` names the phone on the
    /// terminal's tab while its size is in force. The size comes later, from `fit`.
    pub fn watch(&self, id: TermId, label: &str) -> Result<Mirror, Gone> {
        self.shared.links.get(id).filter(|l| l.speaks).ok_or(Gone)?;
        let connected = proto::connect(&dir::sock_path(id)).map_err(|_| Gone)?;
        if connected.major != proto::MAJOR {
            return Err(Gone);
        }
        let stream = connected.stream;
        stream.set_read_timeout(None).map_err(|_| Gone)?;
        let mut input = stream.try_clone().map_err(|_| Gone)?;
        let scrollback = xenon_store::load_service_settings()
            .unwrap_or_default()
            .terminal_scrollback_lines;
        let inner = Arc::new(Inner {
            emu: Mutex::new(Counted::new(FIRST_SIZE, scrollback)),
            flags: Mutex::default(),
            wake: Condvar::new(),
            closed: AtomicBool::new(false),
        });
        let mirror = Mirror {
            out: Mutex::new(stream),
            inner: Arc::clone(&inner),
            label: label.to_owned(),
        };
        mirror.send(&ToHolder::View { answers: false });
        thread::Builder::new()
            .name("xenon-mirror".into())
            .spawn(move || {
                read(&inner, &mut input);
                inner.closed.store(true, Ordering::SeqCst);
                inner.poke();
            })
            .map_err(|_| Gone)?;
        Ok(mirror)
    }
}

fn read(inner: &Inner, input: &mut std::os::unix::net::UnixStream) {
    while let Ok(frame) = proto::read_frame(input) {
        match frame {
            Frame::Data(bytes) => locked(&inner.emu).advance(&bytes),
            Frame::Control(bytes) => match proto::decode::<FromHolder>(&bytes) {
                Control::Body(FromHolder::Size(size)) => locked(&inner.emu).resize(size),
                Control::Body(FromHolder::Screen) => locked(&inner.emu).reset(),
                Control::Body(FromHolder::Status(_)) | Control::Frozen(_) | Control::Unknown => {
                    continue;
                }
            },
        }
        inner.poke();
    }
}

impl Counted {
    fn new(size: Size, cap: usize) -> Self {
        Self {
            emu: Emu::new(size, cap + SLACK),
            count: LineCount { epoch: 0, live: 0 },
            tail: Vec::new(),
            cap,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            history: self.emu.term.grid().history_size(),
            alt: self.emu.mode().contains(TermMode::ALT_SCREEN),
            size: self.emu.size,
        }
    }

    /// Feed output a slice at a time, counting the lines each slice pushes into history.
    fn advance(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(SLICE) {
            let clears = numbering::clears(&self.tail, chunk);
            self.tail = numbering::next_tail(&self.tail, chunk);
            let before = self.probe();
            let _ = self.emu.advance(chunk);
            self.account(before, Cause::Bytes { clears });
        }
    }

    fn resize(&mut self, size: Size) {
        let before = self.probe();
        self.emu.resize(size);
        self.account(before, Cause::Resize);
    }

    fn reset(&mut self) {
        let before = self.probe();
        self.emu = Emu::new(self.emu.size, self.cap + SLACK);
        self.tail.clear();
        self.account(before, Cause::Reset);
    }

    fn account(&mut self, before: Probe, cause: Cause) {
        self.count = numbering::step(self.count, before, self.probe(), cause, self.cap + SLACK);
        // Drop the oldest lines past `cap`, then reopen the limit for the next slice.
        if self.emu.term.grid().history_size() > self.cap {
            let grid = self.emu.term.grid_mut();
            grid.update_history(self.cap);
            grid.update_history(self.cap + SLACK);
        }
    }

    fn numbering(&self) -> Numbering {
        let term = &self.emu.term;
        let history = term.grid().history_size() as u64;
        Numbering {
            epoch: self.count.epoch,
            oldest: self.count.live.saturating_sub(history),
            live: self.count.live,
            rows: term.screen_lines() as u16,
            cols: term.columns() as u16,
            alt: self.emu.mode().contains(TermMode::ALT_SCREEN),
        }
    }
}

impl Inner {
    fn poke(&self) {
        locked(&self.flags).dirty = true;
        self.wake.notify_all();
    }
}

impl Mirror {
    fn send(&self, msg: &ToHolder) {
        let _ = proto::write_control(&mut *locked(&self.out), msg);
    }

    pub fn is_gone(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }

    pub fn wake(&self) {
        self.inner.poke();
    }

    /// Block until the screen changed and `gap` has passed since `last`. False once the
    /// mirror is stopped or its terminal is gone.
    pub fn wait_dirty(&self, last: Instant, gap: Duration) -> bool {
        let mut flags = locked(&self.inner.flags);
        while !flags.dirty && !flags.stop && !self.is_gone() {
            flags = self
                .inner
                .wake
                .wait(flags)
                .unwrap_or_else(|e| e.into_inner());
        }
        let stopped = flags.stop;
        drop(flags);
        if stopped || self.is_gone() {
            return false;
        }
        thread::sleep(gap.saturating_sub(last.elapsed()));
        locked(&self.inner.flags).dirty = false;
        true
    }

    /// Stop for good: wake whoever waits in `wait_dirty` and close the connection, which
    /// gives back the size this phone claimed.
    pub fn stop(&self) {
        locked(&self.inner.flags).stop = true;
        self.inner.wake.notify_all();
        let _ = locked(&self.out).shutdown(std::net::Shutdown::Both);
    }
}

impl Drop for Mirror {
    fn drop(&mut self) {
        self.stop();
    }
}
