//! Terminal backend and view for xero: init the zed globals `terminal` needs,
//! spawn a PTY, and render its grid in a focusable GPUI view.

mod clipboard;
mod init;

// color/grid/view are adapted from zed (see ATTRIBUTION.md). They are exempt
// from our complexity lint gate so that upstream re-syncs stay a deliberate
// choice rather than being forced by our stricter-than-zed thresholds. Do not
// refactor these to satisfy our lints; sync them from upstream instead.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::type_complexity
)]
mod color;
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::type_complexity
)]
mod grid;
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::type_complexity
)]
mod view;

pub use init::{apply_system_theme, init, observe_appearance};
pub use terminal;
pub use view::{TerminalEvent, TerminalView};
