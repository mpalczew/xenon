//! Phone fit rules: while a phone drives a terminal, its grid is sized to the
//! phone's. Local keyboard input hands the grid back for `LOCAL_QUIET`. Both
//! frontends keep one `FitGate` per terminal and apply what it says.

use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

/// Local keyboard quiet time before a phone fit (re)applies.
pub(crate) const LOCAL_QUIET: Duration = Duration::from_secs(30);
/// Phone fit bounds (anything else is a buggy or hostile client).
const FIT_COLS: RangeInclusive<u16> = 20..=300;
const FIT_ROWS: RangeInclusive<u16> = 8..=200;

/// A phone's grid, clamped to sane bounds.
pub fn clamp_fit(cols: u16, rows: u16) -> (u16, u16) {
    (
        cols.clamp(*FIT_COLS.start(), *FIT_COLS.end()),
        rows.clamp(*FIT_ROWS.start(), *FIT_ROWS.end()),
    )
}

/// The smallest grid among the phones on one terminal, with the first one's
/// payload (its device name). None when no phone asked.
pub fn smallest_fit<D>(fits: impl IntoIterator<Item = (u16, u16, D)>) -> Option<(u16, u16, D)> {
    fits.into_iter()
        .reduce(|(c1, r1, d1), (c2, r2, _)| (c1.min(c2), r1.min(r2), d1))
}

/// One terminal's phone fit request plus the Mac's last keystroke.
#[derive(Debug)]
pub struct FitGate<T> {
    request: Option<T>,
    local_input_at: Option<Instant>,
}

impl<T> Default for FitGate<T> {
    fn default() -> Self {
        Self {
            request: None,
            local_input_at: None,
        }
    }
}

impl<T: PartialEq> FitGate<T> {
    /// A phone asked (or let go). True when that changed anything.
    pub fn set(&mut self, request: Option<T>) -> bool {
        if self.request == request {
            return false;
        }
        self.request = request;
        true
    }

    /// What the phone wants, whether or not the Mac is typing.
    pub fn requested(&self) -> Option<&T> {
        self.request.as_ref()
    }

    /// The request in effect: the Mac has been quiet for `LOCAL_QUIET`.
    pub fn active(&self, now: Instant) -> Option<&T> {
        self.request.as_ref().filter(|_| self.quiet(now))
    }

    /// Local keyboard input. True when it just took the grid back from a phone.
    pub fn note_local_input(&mut self, now: Instant) -> bool {
        let was_active = self.active(now).is_some();
        self.local_input_at = Some(now);
        was_active
    }

    /// How long until a waiting request applies (None = nothing waits).
    pub fn wait_left(&self, now: Instant) -> Option<Duration> {
        let at = self.local_input_at.filter(|_| self.request.is_some())?;
        let wait = LOCAL_QUIET.saturating_sub(now.saturating_duration_since(at));
        (!wait.is_zero()).then_some(wait)
    }

    fn quiet(&self, now: Instant) -> bool {
        self.local_input_at
            .is_none_or(|at| now.saturating_duration_since(at) >= LOCAL_QUIET)
    }
}
