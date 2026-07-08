//! A stream of work within a workspace. In v1 a stream is a saved session over
//! the shared checkout; `Backing` is the seam that lets a stream later be
//! backed by an isolated git worktree without touching the UI.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ids::StreamId;
use crate::session::SessionState;

/// What filesystem a stream operates on. Serialized with a `kind` tag so new
/// variants (e.g. `Worktree`) are a forward-compatible addition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Backing {
    /// The workspace's own checkout, shared with every other stream.
    #[default]
    Checkout,
    // Future: Worktree { branch: String, path: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub id: StreamId,
    pub name: String,
    #[serde(default)]
    pub backing: Backing,
    #[serde(default)]
    pub session: SessionState,
}

impl Stream {
    pub fn new(name: impl Into<String>) -> Self {
        Stream {
            id: StreamId::new(),
            name: name.into(),
            backing: Backing::Checkout,
            session: SessionState::default(),
        }
    }

    /// The directory this stream operates in. THE seam: terminal cwd, editor
    /// file resolution, finder root, and file tree root all route through here.
    pub fn working_dir(&self, workspace_root: &Path) -> PathBuf {
        match &self.backing {
            Backing::Checkout => workspace_root.to_path_buf(),
        }
    }
}
