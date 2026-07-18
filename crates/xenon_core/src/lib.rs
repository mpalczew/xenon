//! Pure data model for Xenon: workspaces, session state, and VS Code shell-task
//! parsing. No gpui — persistence lives in `xenon_store`.

mod content;
mod ids;
mod session;
mod tasks;
mod workspace;

pub use content::{
    ContentLayout, DEFAULT_SPLIT_RATIO, DropEdge, LayoutError, LeafPane, LegacySession,
    MAX_NEST_DEPTH, PaneId, PaneNode, SplitAxis, TabId, TabState, migrate_from_legacy,
};
pub use ids::WorkspaceId;
pub use session::{
    DEFAULT_SIDEBAR_WIDTH, DEFAULT_TERMINAL_WIDTH, Layout, OpenEditor, Point, SessionState,
    TerminalState, clamp_sidebar, clamp_terminal,
};
pub use tasks::{
    ShellTask, find_tasks_json, load_shell_tasks, parse_shell_tasks, workspace_for_tasks_json,
};
pub use workspace::{Active, CURRENT_VERSION, Registry, WorkspaceRec};

#[cfg(test)]
mod tests;
