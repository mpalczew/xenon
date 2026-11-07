//! Pure keymap model: per-platform default bindings, the JSON the shell feeds
//! to gpui, and shortcut text for the current platform. No gpui in here, so
//! all of it is unit-tested.

mod chord;
mod defaults;
mod display;
mod effective;

pub use chord::Platform;
pub use defaults::defaults;
pub use display::{display_keys, display_text};

/// One binding: `action` of `None` unbinds the keystroke (JSON `null`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub context: Option<String>,
    pub keystroke: String,
    pub action: Option<String>,
}

impl Entry {
    pub fn new(context: Option<&str>, keystroke: &str, action: Option<&str>) -> Self {
        Self {
            context: context.map(str::to_owned),
            keystroke: keystroke.to_owned(),
            action: action.map(str::to_owned),
        }
    }
}

pub(crate) use chord::Chord;
