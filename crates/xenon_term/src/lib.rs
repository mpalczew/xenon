//! The terminal service and its client (no GTK).
//!
//! Spike scope: PTY spawn, one reader thread per terminal, an `alacritty_terminal`
//! emulator with `replay()`, and a minimal socket service with create, attach, input,
//! resize and close. See `docs/designs/terminal-service.md`.

mod calls;
mod client;
mod codec;
mod dec;
mod emu;
mod pen;
mod service;
mod stubs;
mod wire;

use std::fmt;
use std::path::{Path, PathBuf};

pub use client::{Client, Event, TermInfo};
pub use service::run_service;

/// Random and never reused: a stale id only misses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TermId(pub(crate) u128);

impl TermId {
    pub(crate) fn random() -> Self {
        Self(uuid::Uuid::new_v4().as_u128())
    }
}

impl fmt::Display for TermId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// Columns and rows, both at least 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub(crate) cols: u16,
    pub(crate) rows: u16,
}

impl Size {
    pub fn new(cols: u16, rows: u16) -> Option<Self> {
        (cols >= 1 && rows >= 1).then_some(Self { cols, rows })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
}

/// What to run. Empty `argv` runs the login shell. `env` (as `KEY=VALUE`) is added to
/// the base environment the window sent on connect.
#[derive(Clone, Debug)]
pub struct Spawn {
    pub cwd: PathBuf,
    pub argv: Vec<String>,
    pub env: Vec<String>,
}

impl Spawn {
    pub fn shell(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_owned(),
            argv: Vec::new(),
            env: Vec::new(),
        }
    }
}

/// Where the spike's service listens unless told otherwise.
pub fn default_socket() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    dir.join("xenon-term-spike.sock")
}
