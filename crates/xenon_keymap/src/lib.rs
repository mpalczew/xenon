//! Pure keymap model: per-platform default bindings, the JSON the shell feeds
//! to gpui, and shortcut text for the current platform. No gpui in here, so
//! all of it is unit-tested.

mod chord;
mod defaults;
mod display;
mod effective;

pub use chord::{Chord, Platform};
pub use defaults::{defaults, to_json};
pub use display::{display_keys, display_text, set_display_overrides};
pub use effective::display_overrides;

/// File content written by "Open Keymap" when the file does not exist yet.
pub const STARTER_KEYMAP: &str = "[\n  {\n    \"bindings\": {}\n  }\n]\n";

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
