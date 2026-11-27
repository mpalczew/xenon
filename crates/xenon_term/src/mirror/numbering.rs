//! Line numbers for a mirrored terminal. alacritty does not count lines entering history
//! or being trimmed, so the mirror measures history growth around each slice of bytes and
//! starts a new epoch whenever the count can't be known exactly.

use crate::Size;

/// How the mirror's grid maps to line numbers right now. Within one `epoch` a number
/// always names the same line: screen row r is `live + r`, history depth k is `live - k`.
/// History lines (below `live`) never change within an epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Numbering {
    pub epoch: u32,
    /// The oldest history line still held; anything below it was trimmed.
    pub oldest: u64,
    /// Screen row 0.
    pub live: u64,
    pub rows: u16,
    pub cols: u16,
    /// Alt screen: there is no history, so `oldest == live`.
    pub alt: bool,
}

impl Numbering {
    /// One past the last screen row.
    pub fn bottom(&self) -> u64 {
        self.live + u64::from(self.rows)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LineCount {
    pub epoch: u32,
    pub live: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct Probe {
    pub history: usize,
    pub alt: bool,
    pub size: Size,
}

pub(crate) enum Cause {
    Bytes { clears: bool },
    Resize,
    Reset,
}

pub(crate) const SLACK: usize = 4096;
/// Bytes per slice. A byte adds at most one line, except `CSI n S`, which saturation
/// catches.
pub(crate) const SLICE: usize = SLACK;

pub(crate) fn step(
    count: LineCount,
    before: Probe,
    after: Probe,
    cause: Cause,
    limit: usize,
) -> LineCount {
    let fresh = LineCount {
        epoch: count.epoch.wrapping_add(1),
        live: after.history as u64,
    };
    let unknowable = match cause {
        Cause::Reset | Cause::Bytes { clears: true } => true,
        Cause::Resize => before.size != after.size,
        Cause::Bytes { clears: false } => false,
    };
    if unknowable || before.alt != after.alt || after.history >= limit {
        return fresh;
    }
    match after.history.checked_sub(before.history) {
        Some(grew) => LineCount {
            live: count.live + grew as u64,
            ..count
        },
        None => fresh,
    }
}

pub(crate) const TAIL: usize = 3;

pub(crate) fn clears(tail: &[u8], chunk: &[u8]) -> bool {
    let head = &chunk[..chunk.len().min(TAIL)];
    let seam: Vec<u8> = tail.iter().chain(head).copied().collect();
    [&seam[..], chunk]
        .iter()
        .any(|b| b.windows(4).any(|w| w == b"\x1b[3J") || b.windows(2).any(|w| w == b"\x1bc"))
}

pub(crate) fn next_tail(tail: &[u8], chunk: &[u8]) -> Vec<u8> {
    let keep = TAIL.saturating_sub(chunk.len()).min(tail.len());
    let mut out = tail[tail.len() - keep..].to_vec();
    out.extend_from_slice(&chunk[chunk.len().saturating_sub(TAIL)..]);
    out
}
