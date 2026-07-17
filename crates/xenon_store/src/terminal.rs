//! Terminal-related durable settings.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Close a terminal tab after its shell process exits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalAutoClose {
    Off,
    Immediate,
    #[default]
    After1s,
    After3s,
    After5s,
}

impl TerminalAutoClose {
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Immediate,
        Self::After1s,
        Self::After3s,
        Self::After5s,
    ];

    /// `None` = leave the tab open; `Some(d)` = close after `d` (zero = now).
    pub fn delay(self) -> Option<Duration> {
        match self {
            Self::Off => None,
            Self::Immediate => Some(Duration::ZERO),
            Self::After1s => Some(Duration::from_secs(1)),
            Self::After3s => Some(Duration::from_secs(3)),
            Self::After5s => Some(Duration::from_secs(5)),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Keep open",
            Self::Immediate => "Immediately",
            Self::After1s => "After 1 second",
            Self::After3s => "After 3 seconds",
            Self::After5s => "After 5 seconds",
        }
    }

    /// Compact label for the terminal status strip.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Off => "Keep open",
            Self::Immediate => "Now",
            Self::After1s => "1s",
            Self::After3s => "3s",
            Self::After5s => "5s",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Immediate,
            Self::Immediate => Self::After1s,
            Self::After1s => Self::After3s,
            Self::After3s => Self::After5s,
            Self::After5s => Self::Off,
        }
    }
}
