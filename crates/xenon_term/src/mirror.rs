use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use xenon_core::TermId;

use crate::hold::{Emu, FIRST_SIZE};
use crate::proto::v1::{FromHolder, ToHolder};
use crate::proto::{self, Control, Frame};
use crate::{Size, Terminals, dir};

/// The terminal is gone, or its holder speaks a protocol this build cannot read.
#[derive(Debug)]
pub struct Gone;

mod calls;

#[derive(Clone, Copy, Debug)]
pub struct Modes {
    pub size: Size,
    pub app_cursor: bool,
    pub mouse: bool,
    pub sgr_mouse: bool,
}

#[derive(Default)]
struct Flags {
    dirty: bool,
    stop: bool,
}

struct Inner {
    emu: Mutex<Emu>,
    scroll: AtomicUsize,
    flags: Mutex<Flags>,
    wake: Condvar,
    closed: AtomicBool,
    scrollback: usize,
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
        self.shared.link(id).filter(|l| l.speaks).ok_or(Gone)?;
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
            emu: Mutex::new(Emu::new(FIRST_SIZE, scrollback)),
            scroll: AtomicUsize::new(0),
            flags: Mutex::default(),
            wake: Condvar::new(),
            closed: AtomicBool::new(false),
            scrollback,
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
            Frame::Data(bytes) => {
                let _ = locked(&inner.emu).advance(&bytes);
            }
            Frame::Control(bytes) => match proto::decode::<FromHolder>(&bytes) {
                Control::Body(FromHolder::Size(size)) => locked(&inner.emu).resize(size),
                Control::Body(FromHolder::Screen) => {
                    let mut emu = locked(&inner.emu);
                    let size = emu.size;
                    *emu = Emu::new(size, inner.scrollback);
                }
                Control::Body(FromHolder::Status(_)) | Control::Frozen(_) | Control::Unknown => {
                    continue;
                }
            },
        }
        inner.poke();
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
