//! Terminal backend and view for xero: init the zed globals `terminal` needs,
//! spawn a PTY, and render its grid in a focusable GPUI view.

mod color;
mod grid;
mod init;
mod view;

pub use init::{apply_system_theme, init, observe_appearance};
pub use terminal;
pub use view::TerminalView;
