//! Who wants a terminal at what size, merged at read time.
//!
//! The desk's size wins, unless a phone has claimed one and the desk has had no input for
//! 30 seconds; then the smallest phone wins. Nothing writes the size directly: it is
//! derived from the claims.

use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

use crate::{Claim, Size};

/// How long the desk must be idle before a phone's size takes over.
const DESK_QUIET: Duration = Duration::from_secs(30);
const PHONE_COLS: RangeInclusive<u16> = 20..=300;
const PHONE_ROWS: RangeInclusive<u16> = 8..=200;

#[derive(Default)]
pub(crate) struct Claims {
    desk: Option<Size>,
    /// Each phone connection's size and the name its tab shows.
    phones: Vec<(u64, Size, String)>,
    desk_input_at: Option<Instant>,
}

impl Claims {
    pub(crate) fn set(&mut self, conn: u64, claim: Claim) {
        match claim {
            Claim::Desk(size) => self.desk = Some(size),
            Claim::Phone { size, label } => {
                let cols = size.cols().clamp(*PHONE_COLS.start(), *PHONE_COLS.end());
                let rows = size.rows().clamp(*PHONE_ROWS.start(), *PHONE_ROWS.end());
                let size = Size { cols, rows };
                match self.phones.iter_mut().find(|(c, ..)| *c == conn) {
                    Some(entry) => *entry = (conn, size, label),
                    None => self.phones.push((conn, size, label)),
                }
            }
        }
    }

    /// A connection went away: forget what it claimed.
    pub(crate) fn drop_conn(&mut self, conn: u64) {
        self.phones.retain(|(c, ..)| *c != conn);
    }

    /// The smallest phone size, named for the first phone that claimed one.
    fn smallest(&self) -> Option<(Size, &str)> {
        let (_, first, label) = self.phones.first()?;
        let size = self.phones.iter().fold(*first, |acc, (_, s, _)| Size {
            cols: acc.cols.min(s.cols),
            rows: acc.rows.min(s.rows),
        });
        Some((size, label))
    }

    /// The phone size in force now: only while the desk is quiet.
    fn active(&self, now: Instant) -> Option<(Size, &str)> {
        let quiet = self
            .desk_input_at
            .is_none_or(|at| now.saturating_duration_since(at) >= DESK_QUIET);
        self.smallest().filter(|_| quiet)
    }

    /// The desk typed. True when that took the size back from a phone.
    pub(crate) fn note_desk_input(&mut self, now: Instant) -> bool {
        let was_active = self.active(now).is_some();
        self.desk_input_at = Some(now);
        was_active
    }

    /// The size the terminal should have now; `current` when nobody has a claim.
    pub(crate) fn effective(&self, now: Instant, current: Size) -> Size {
        match self.active(now) {
            Some((size, _)) => size,
            None => self.desk.unwrap_or(current),
        }
    }

    /// The phone whose size is in force now.
    pub(crate) fn active_label(&self, now: Instant) -> Option<String> {
        self.active(now).map(|(_, label)| label.to_owned())
    }

    /// How long until the desk's quiet period ends and a waiting phone takes over.
    pub(crate) fn wait_left(&self, now: Instant) -> Option<Duration> {
        let at = self.desk_input_at.filter(|_| !self.phones.is_empty())?;
        let wait = DESK_QUIET.saturating_sub(now.saturating_duration_since(at));
        (!wait.is_zero()).then_some(wait)
    }
}
