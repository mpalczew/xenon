//! Pure data model for xero: workspaces, streams, and session state. No gpui,
//! no I/O — persistence lives in `xero_store`.

mod ids;
mod session;
mod stream;
mod workspace;

pub use ids::{StreamId, WorkspaceId};
pub use session::{
    DEFAULT_SIDEBAR_WIDTH, DEFAULT_TERMINAL_WIDTH, DEFAULT_TREE_WIDTH, Layout, OpenEditor, Point,
    SessionState, TerminalState, clamp_sidebar, clamp_terminal, clamp_tree,
};
pub use stream::{Backing, Stream};
pub use workspace::{Active, CURRENT_VERSION, Registry, WorkspaceRec};

#[cfg(test)]
mod tests;
