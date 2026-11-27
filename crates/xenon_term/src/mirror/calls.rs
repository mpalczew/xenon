use alacritty_terminal::Term;
use alacritty_terminal::term::TermMode;

use super::{Mirror, Modes, Numbering, locked};
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

    /// Read the mirrored terminal at its live edge, with its line numbering.
    pub fn read<R>(&self, f: impl FnOnce(&Term<Listener>, Numbering) -> R) -> R {
        let counted = locked(&self.inner.emu);
        f(&counted.emu.term, counted.numbering())
    }

    pub fn modes(&self) -> Modes {
        let counted = locked(&self.inner.emu);
        let emu = &counted.emu;
        let mode = emu.mode();
        Modes {
            size: emu.size,
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            mouse: mode.intersects(
                TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION,
            ),
            sgr_mouse: mode.contains(TermMode::SGR_MOUSE),
            alt: mode.contains(TermMode::ALT_SCREEN),
            alt_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
        }
    }
}
