use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

pub(crate) const LOCAL_QUIET: Duration = Duration::from_secs(30);
const FIT_COLS: RangeInclusive<u16> = 20..=300;
const FIT_ROWS: RangeInclusive<u16> = 8..=200;

pub fn clamp_fit(cols: u16, rows: u16) -> (u16, u16) {
    (
        cols.clamp(*FIT_COLS.start(), *FIT_COLS.end()),
        rows.clamp(*FIT_ROWS.start(), *FIT_ROWS.end()),
    )
}

pub fn smallest_fit<D>(fits: impl IntoIterator<Item = (u16, u16, D)>) -> Option<(u16, u16, D)> {
    fits.into_iter()
        .reduce(|(c1, r1, d1), (c2, r2, _)| (c1.min(c2), r1.min(r2), d1))
}

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
    pub fn set(&mut self, request: Option<T>) -> bool {
        if self.request == request {
            return false;
        }
        self.request = request;
        true
    }

    pub fn requested(&self) -> Option<&T> {
        self.request.as_ref()
    }

    pub fn active(&self, now: Instant) -> Option<&T> {
        self.request.as_ref().filter(|_| self.quiet(now))
    }

    pub fn note_local_input(&mut self, now: Instant) -> bool {
        let was_active = self.active(now).is_some();
        self.local_input_at = Some(now);
        was_active
    }

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
