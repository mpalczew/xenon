//! Which lines the phone should hold. Pure: the frame thread plans, renders what's
//! missing and encodes it.

use std::ops::Range;

use xenon_term::Numbering;

/// Where the phone is scrolled. Latest wins.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Report {
    pub epoch: u32,
    /// The line at the top of the phone's viewport.
    pub top: u64,
    pub follow: bool,
    /// Lines the viewport shows.
    pub lines: u16,
}

/// Preload, in screens.
#[derive(Clone, Copy)]
pub(crate) struct Depth {
    above: u64,
    below: u64,
    cap: u64,
}

impl Depth {
    pub fn of(low_bandwidth: bool) -> Self {
        if low_bandwidth {
            Self {
                above: 1,
                below: 1,
                cap: 3,
            }
        } else {
            Self {
                above: 3,
                below: 1,
                cap: 8,
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Want {
    pub range: Range<u64>,
    /// Put this line at the viewport top. Set only when a new epoch re-anchors a reader.
    pub anchor: Option<u64>,
}

enum Pos {
    Follow,
    At(u64),
    /// The phone hasn't reported in this epoch yet: keep what it has.
    Stay,
}

/// One contiguous range around the phone's position. The current range is kept while it
/// still covers a comfort zone around the view, so a slow scroll doesn't churn.
pub(crate) fn plan(
    at: Numbering,
    report: Option<Report>,
    held: Option<(Numbering, Range<u64>)>,
    d: Depth,
) -> Want {
    let bottom = at.bottom();
    if at.alt {
        return Want {
            range: at.live..bottom,
            anchor: None,
        };
    }
    let held = held.filter(|(h, _)| !h.alt);
    let new_epoch = held.as_ref().is_some_and(|(h, _)| h.epoch != at.epoch);
    let pos = match (report, &held) {
        (Some(p), _) if p.follow => Pos::Follow,
        (Some(p), _) if p.epoch == at.epoch => Pos::At(p.top.clamp(at.oldest, at.live)),
        (Some(p), Some((h, _))) if new_epoch => Pos::At(carry(*h, at, p.top)),
        (Some(_), Some(_)) => Pos::Stay,
        _ => Pos::Follow,
    };
    let kept = held.filter(|_| !new_epoch).and_then(|(h, range)| {
        let end = if range.end >= h.live {
            bottom
        } else {
            range.end
        };
        let start = range.start.max(at.oldest);
        (start < end).then_some(start..end)
    });
    let (anchor, follow) = match pos {
        Pos::Follow => (at.live, true),
        Pos::At(top) => (top, false),
        Pos::Stay => match kept {
            Some(range) => {
                return Want {
                    range,
                    anchor: None,
                };
            }
            None => (at.live, true),
        },
    };
    // A screen is what the phone shows: more than `rows` while it shrinks a wide terminal.
    let r = u64::from(at.rows.max(report.map_or(0, |p| p.lines)));
    let need = anchor.saturating_sub(d.above * r / 2).max(at.oldest)
        ..(anchor + r + d.below * r / 2).min(bottom);
    if let Some(range) = kept
        && range.start <= need.start
        && range.end >= need.end
        && range.end - range.start <= d.cap * r
    {
        return Want {
            range,
            anchor: None,
        };
    }
    let lo = anchor.saturating_sub(d.above * r).max(at.oldest);
    let hi = if follow {
        bottom
    } else {
        (anchor + (1 + d.below) * r).min(bottom)
    };
    Want {
        range: lo..hi,
        anchor: (new_epoch && !follow).then_some(anchor),
    }
}

/// Carry a reading position into a new epoch by its distance from the live edge.
/// Approximate across a reflow, where wrapped lines change count.
fn carry(old: Numbering, new: Numbering, top: u64) -> u64 {
    let from_end = old.bottom().saturating_sub(top);
    new.bottom()
        .saturating_sub(from_end)
        .clamp(new.oldest, new.live)
}
