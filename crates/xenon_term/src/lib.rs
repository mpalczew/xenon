mod builds;
mod calls;
mod dir;
mod end;
mod hold;
mod holder_link;
mod links;
mod look;
mod mirror;
mod mirror_view;
mod proto;
mod start;
mod terminals;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use builds::installed_build;
pub use dir::{phone_socket_path, watch_dir};
pub use end::end_all;
pub use hold::run_holder;
pub use look::Look;
pub use mirror::{Gone, Mirror, Modes};
pub use terminals::{Event, Terminals};
pub use xenon_core::{TermId, WorkspaceId};

use xenon_core::AttentionReason;

pub const BUILD: &str = env!("XENON_BUILD");

/// Columns and rows, both at least 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawSize")]
pub struct Size {
    cols: u16,
    rows: u16,
}

#[derive(Deserialize)]
struct RawSize {
    cols: u16,
    rows: u16,
}

impl TryFrom<RawSize> for Size {
    type Error = &'static str;

    fn try_from(raw: RawSize) -> Result<Self, Self::Error> {
        Self::new(raw.cols, raw.rows).ok_or("size must be at least 1x1")
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
/// the base environment `Terminals::start` was given.
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

#[derive(Clone, Debug)]
pub(crate) enum Claim {
    Desk(Size),
    Phone { size: Size, label: String },
}

/// What dots, titles and exit marks are painted from. Derived in the holder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TermStatus {
    pub working: bool,
    pub attention: Option<AttentionReason>,
    pub title: String,
    pub exit: Option<Exit>,
    pub phone: Option<String>,
}

/// Which build a terminal runs on. Worked out when read, never stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Build {
    pub label: String,
    /// The installed `xenon-term` is not the binary this holder is running.
    pub older: bool,
    /// This build can read the holder's protocol. When false the terminal can be listed
    /// and closed, nothing else.
    pub speaks: bool,
}

#[derive(Clone, Debug)]
pub struct TermInfo {
    pub id: TermId,
    pub workspace: WorkspaceId,
    pub pid: i32,
    pub cwd: String,
    pub status: TermStatus,
    pub build: Build,
}
