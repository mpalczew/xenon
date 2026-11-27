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

/// The counted state: everything else is derived from the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Log {
    pub epoch: u32,
    pub live: u64,
}

/// What the mirror measures around one step.
#[derive(Clone, Copy)]
pub(crate) struct Probe {
    pub history: usize,
    pub alt: bool,
    pub size: Size,
}

pub(crate) enum Cause {
    /// A slice of output. `clears` is true when it held CSI 3J or RIS.
    Bytes {
        clears: bool,
    },
    Resize,
    /// The emulator was rebuilt before a replay.
    Reset,
}

/// History may grow by this much in one slice before the count is unknowable.
pub(crate) const SLACK: usize = 4096;
/// Bytes per slice. A byte adds at most one line, except `CSI n S`, which saturation
/// catches.
pub(crate) const SLICE: usize = SLACK;

/// Accounts one step. Anything that can't be measured exactly starts a new epoch, so a
/// wrong number is never served. `limit` is the emulator's scrollback limit.
pub(crate) fn step(log: Log, before: Probe, after: Probe, cause: Cause, limit: usize) -> Log {
    let fresh = Log {
        epoch: log.epoch.wrapping_add(1),
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
        Some(grew) => Log {
            live: log.live + grew as u64,
            ..log
        },
        None => fresh,
    }
}

/// Bytes of the previous slice kept so a sequence split across slices is still seen.
pub(crate) const TAIL: usize = 3;

/// Does `tail ++ chunk` hold `ESC [ 3 J` or `ESC c`?
pub(crate) fn clears(tail: &[u8], chunk: &[u8]) -> bool {
    let head = &chunk[..chunk.len().min(TAIL)];
    let seam: Vec<u8> = tail.iter().chain(head).copied().collect();
    [&seam[..], chunk]
        .iter()
        .any(|b| b.windows(4).any(|w| w == b"\x1b[3J") || b.windows(2).any(|w| w == b"\x1bc"))
}

/// The last `TAIL` bytes of `tail ++ chunk`.
pub(crate) fn next_tail(tail: &[u8], chunk: &[u8]) -> Vec<u8> {
    let keep = TAIL.saturating_sub(chunk.len()).min(tail.len());
    let mut out = tail[tail.len() - keep..].to_vec();
    out.extend_from_slice(&chunk[chunk.len().saturating_sub(TAIL)..]);
    out
}
