//! What a phone does to its mirror: type, size, scroll and read the screen.

use std::sync::atomic::Ordering;

use alacritty_terminal::term::TermMode;
use xenon_remote::Screen;

use super::{Mirror, Modes, locked};
use crate::look::Look;
use crate::proto::v1::ToHolder;
use crate::{Size, mirror_view, proto};

impl Mirror {
    /// Keys, pastes and mouse reports for the program.
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
        let max = mirror_view::history_size(&locked(&self.inner.emu)) as i64;
        let now = self.inner.scroll.load(Ordering::Relaxed) as i64;
        let next = (now + i64::from(rows)).clamp(0, max) as usize;
        self.inner.scroll.store(next, Ordering::Relaxed);
        self.inner.poke();
    }

    pub fn bottom(&self) {
        self.inner.scroll.store(0, Ordering::Relaxed);
        self.inner.poke();
    }

    /// The screen at this phone's own scroll position.
    pub fn screen(&self, look: &Look) -> Screen {
        let offset = self.inner.scroll.load(Ordering::Relaxed);
        mirror_view::screen(&locked(&self.inner.emu), offset, look)
    }

    /// Up to `limit` scrollback lines, oldest first, as plain text.
    pub fn history(&self, limit: usize) -> Vec<String> {
        mirror_view::history_lines(&locked(&self.inner.emu), limit)
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
