//! The terminal service and its client (no GTK).
//!
//! `xenon-term` runs and holds every terminal; the window and the phone page are its
//! clients. See `docs/designs/terminal-service.md`.

mod calls;
mod claims;
mod client;
mod codec;
mod conn;
mod dec;
mod emu;
mod emu_replay;
mod emu_view;
mod hub;
mod pen;
mod phone;
mod phone_api;
mod phone_link;
mod pty;
mod reader;
mod service;
mod status;
mod term;
mod terms;
mod wire;

use std::path::{Path, PathBuf};

pub use client::{Client, Event};
pub use phone_api::Pairing;
pub use phone_link::PhoneLink;
pub use service::run_service;
pub use xenon_core::{TermId, WorkspaceId};

use xenon_core::AttentionReason;

/// Columns and rows, both at least 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    cols: u16,
    rows: u16,
}

impl Size {
    pub fn new(cols: u16, rows: u16) -> Option<Self> {
        (cols >= 1 && rows >= 1).then_some(Self { cols, rows })
    }

    pub fn cols(self) -> u16 {
        self.cols
    }

    pub fn rows(self) -> u16 {
        self.rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
}

impl Exit {
    /// The exit code a shell would report: a signal counts as 128 plus its number.
    pub fn code(self) -> i32 {
        match self {
            Self::Code(code) => code,
            Self::Signal(signal) => 128 + signal,
        }
    }
}

/// What to run. Empty `argv` runs the user's shell. `env` (as `KEY=VALUE`) is added to
/// the base environment the window sent on connect.
#[derive(Clone, Debug, Default)]
pub struct Spawn {
    pub cwd: PathBuf,
    pub argv: Vec<String>,
    pub env: Vec<String>,
}

impl Spawn {
    pub fn shell(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_owned(),
            ..Self::default()
        }
    }
}

/// Who wants the terminal at what size. The window is always the desk; phones claim
/// inside the service.
#[derive(Clone, Debug)]
pub(crate) enum Claim {
    Desk(Size),
    Phone { size: Size, label: String },
}

/// What dots, titles and exit marks are painted from. Derived in the service.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TermStatus {
    pub working: bool,
    pub attention: Option<AttentionReason>,
    /// The cleaned title, ready for a tab label.
    pub title: String,
    pub exit: Option<Exit>,
    /// The phone that currently sets this terminal's size, by short name.
    pub phone: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TermInfo {
    pub id: TermId,
    pub workspace: WorkspaceId,
    pub pid: i32,
    pub status: TermStatus,
}

/// Where the service listens: `XENON_TERM_SOCKET`, else `$XDG_RUNTIME_DIR/xenon-term.sock`.
pub(crate) fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("XENON_TERM_SOCKET") {
        return PathBuf::from(path);
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    dir.join("xenon-term.sock")
}
