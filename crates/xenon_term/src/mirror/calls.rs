use std::sync::atomic::Ordering;

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::TermMode;

use super::{Mirror, Modes, locked};
use crate::hold::Listener;
use crate::proto::v1::ToHolder;
use crate::{Size, proto};

impl Mirror {
    pub fn input(&self, bytes: &[u8]) {
        let _ = proto::write_data(&mut *locked(&self.out), bytes);
    }

    /// This phone's screen size: the terminal takes it while the computer is idle.
    pub fn fit(&self, size: Size) {
        self.send(&ToHolder::Claim {
            size,
            label: Some(self.label.clone()),
        });
    }

    /// Move the view `rows` up (negative: down) in the scrollback.
    pub fn scroll(&self, rows: i32) {
        let max = locked(&self.inner.emu).term.grid().history_size() as i64;
        let now = self.inner.scroll.load(Ordering::Relaxed) as i64;
        let next = (now + i64::from(rows)).clamp(0, max) as usize;
        self.inner.scroll.store(next, Ordering::Relaxed);
        self.inner.poke();
    }

    pub fn bottom(&self) {
        self.inner.scroll.store(0, Ordering::Relaxed);
        self.inner.poke();
    }

    /// Read the mirrored terminal and how many rows the view is scrolled up.
    pub fn read<R>(&self, f: impl FnOnce(&Term<Listener>, usize) -> R) -> R {
        let offset = self.inner.scroll.load(Ordering::Relaxed);
        f(&locked(&self.inner.emu).term, offset)
    }

    pub fn modes(&self) -> Modes {
        let emu = locked(&self.inner.emu);
        let mode = emu.mode();
        Modes {
            size: emu.size,
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            mouse: mode.intersects(
                TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION,
            ),
            sgr_mouse: mode.contains(TermMode::SGR_MOUSE),
        }
    }
}
