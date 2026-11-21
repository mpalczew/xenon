//! Major 1: the messages after the handshake. Delete when no holder of major 1 lives
//! (the header-bar indicator reads zero older terminals, and no window needs them).
//!
//! Output and input are data frames, not messages here.

use serde::{Deserialize, Serialize};

use crate::{Size, TermStatus};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub(crate) enum ToHolder {
    /// Start output: `Size`, then `Screen` and the replay, then live output. `answers`
    /// says this viewer replies to terminal queries (the window's VTE does).
    View {
        #[serde(default)]
        answers: bool,
    },
    Unview,
    /// This connection's wish for the terminal's size. No label is the window; a label
    /// is a phone, named for its tab.
    Claim {
        size: Size,
        #[serde(default)]
        label: Option<String>,
    },
    Seen,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", rename_all = "snake_case")]
pub(crate) enum FromHolder {
    Status(TermStatus),
    Size(Size),
    /// Reset the view; the replay that rebuilds the screen follows as data.
    Screen,
}
