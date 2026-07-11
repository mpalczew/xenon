//! Pure data model for xero: workspaces, streams, session state, and VS Code
//! shell-task parsing. No gpui — persistence lives in `xero_store`.

mod ids;
mod session;
mod stream;
mod tasks;
mod workspace;

pub use ids::{StreamId, WorkspaceId};
pub use session::{
    DEFAULT_SIDEBAR_WIDTH, DEFAULT_TERMINAL_WIDTH, DEFAULT_TREE_WIDTH, Layout, OpenEditor, Point,
    SessionState, TerminalState, clamp_sidebar, clamp_terminal, clamp_tree,
};
pub use stream::{Backing, Stream};
pub use tasks::{
    ShellTask, find_tasks_json, load_shell_tasks, parse_shell_tasks, workspace_for_tasks_json,
};
pub use workspace::{Active, CURRENT_VERSION, Registry, WorkspaceRec};

#[cfg(test)]
mod tests;
