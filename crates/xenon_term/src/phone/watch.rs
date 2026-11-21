//! A phone watching one terminal: woken when the screen changes, at most once per gap.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Flags {
    dirty: bool,
    stop: bool,
}

#[derive(Default)]
pub(crate) struct Watch {
    flags: Mutex<Flags>,
    wake: Condvar,
}

impl Watch {
    /// The screen changed.
    pub(crate) fn poke(&self) {
        self.flags.lock().unwrap_or_else(|e| e.into_inner()).dirty = true;
        self.wake.notify_all();
    }

    pub(crate) fn stop(&self) {
        self.flags.lock().unwrap_or_else(|e| e.into_inner()).stop = true;
        self.wake.notify_all();
    }

    /// Block until the screen changed and `gap` has passed since `last`. False once
    /// stopped.
    pub(crate) fn wait(&self, last: Instant, gap: Duration) -> bool {
        let mut flags = self.flags.lock().unwrap_or_else(|e| e.into_inner());
        while !flags.dirty && !flags.stop {
            flags = self.wake.wait(flags).unwrap_or_else(|e| e.into_inner());
        }
        drop(flags);
        std::thread::sleep(gap.saturating_sub(last.elapsed()));
        let mut flags = self.flags.lock().unwrap_or_else(|e| e.into_inner());
        flags.dirty = false;
        !flags.stop
    }
}
