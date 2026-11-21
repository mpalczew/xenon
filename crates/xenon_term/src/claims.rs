//! Who wants a terminal at what size, merged at read time.
//!
//! The desk's size wins, unless a phone has claimed one and the desk has had no input for
//! 30 seconds (`FitGate`); then the smallest phone wins. Nothing writes the size
//! directly: it is derived from the claims.

use std::time::{Duration, Instant};

use xenon_remote::{FitGate, clamp_fit, smallest_fit};

use crate::{Claim, Size};

/// A phone's size request and the name its tab shows.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fit {
    cols: u16,
    rows: u16,
    label: String,
}

#[derive(Default)]
pub(crate) struct Claims {
    desk: Option<Size>,
    phones: Vec<(u64, Fit)>,
    gate: FitGate<Fit>,
}

impl Claims {
    pub(crate) fn set(&mut self, conn: u64, claim: Claim) {
        match claim {
            Claim::Desk(size) => self.desk = Some(size),
            Claim::Phone { size, label } => {
                let (cols, rows) = clamp_fit(size.cols(), size.rows());
                let fit = Fit { cols, rows, label };
                match self.phones.iter_mut().find(|(c, _)| *c == conn) {
                    Some(entry) => entry.1 = fit,
                    None => self.phones.push((conn, fit)),
                }
                self.merge();
            }
        }
    }

    /// A connection went away: forget what it claimed.
    pub(crate) fn drop_conn(&mut self, conn: u64) {
        self.phones.retain(|(c, _)| *c != conn);
        self.merge();
    }

    fn merge(&mut self) {
        let smallest = smallest_fit(
            self.phones
                .iter()
                .map(|(_, f)| (f.cols, f.rows, f.label.clone())),
        )
        .map(|(cols, rows, label)| Fit { cols, rows, label });
        self.gate.set(smallest);
    }

    /// The desk typed. True when that took the size back from a phone.
    pub(crate) fn note_desk_input(&mut self, now: Instant) -> bool {
        self.gate.note_local_input(now)
    }

    /// The size the terminal should have now; `current` when nobody has a claim.
    pub(crate) fn effective(&self, now: Instant, current: Size) -> Size {
        match self.gate.active(now) {
            Some(f) => Size::new(f.cols, f.rows).unwrap_or(current),
            None => self.desk.unwrap_or(current),
        }
    }

    /// The phone whose size is in force now.
    pub(crate) fn active_label(&self, now: Instant) -> Option<String> {
        self.gate.active(now).map(|f| f.label.clone())
    }

    /// How long until the desk's quiet period ends and a waiting phone takes over.
    pub(crate) fn wait_left(&self, now: Instant) -> Option<Duration> {
        self.gate.wait_left(now)
    }
}
