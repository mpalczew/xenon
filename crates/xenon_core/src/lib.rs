//! Pure data model for Xenon: workspaces, session state, and VS Code shell-task
//! parsing. No gpui — persistence lives in `xenon_store`.

mod content;
mod ids;
mod session;
mod split;
mod tasks;
mod workspace;

pub use content::{
    ContentLayout, DropEdge, LeafPane, MAX_NEST_DEPTH, MAX_SPLIT_CHILDREN, MIN_SIZE, PaneId,
    PaneNode, SplitAxis, TabId, TabState,
};
pub use ids::WorkspaceId;
pub use session::{DEFAULT_SIDEBAR_WIDTH, Point, SessionState, clamp_sidebar};
pub use split::{SplitTree, flatten_split, move_boundary, normalize_sizes};
pub use tasks::{
    ShellTask, find_tasks_json, load_shell_tasks, parse_shell_tasks, workspace_for_tasks_json,
};
pub use workspace::{Active, CURRENT_VERSION, Registry, WorkspaceRec};

#[cfg(test)]
mod tests;
