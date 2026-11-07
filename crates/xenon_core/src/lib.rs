//! Pure data model for Xenon: workspaces, session state, and VS Code shell-task
//! parsing. No gpui — persistence lives in `xenon_store`.

mod activity;
mod agent;
mod attention;
mod attention_map;
mod content;
mod ids;
mod pane_content;
mod pane_find;
mod pane_moves;
mod pane_new_splits;
mod pane_split;
mod pane_tree;
mod route;
mod session;
mod split;
mod tab_label;
mod tasks;
mod workspace;

pub use activity::TerminalActivity;
pub use agent::{ANONYMOUS_BUSY_WAKEUPS, BUSY_WAKEUPS, IDLE_AFTER, agent_busy_signal, agent_title};
pub use attention::{AttentionReason, WorkspaceDot, workspace_dot, workspace_dot_from_tabs};
pub use attention_map::AttentionMap;
pub use content::{
    ContentLayout, DropEdge, LeafPane, MAX_NEST_DEPTH, MAX_SPLIT_CHILDREN, MIN_SIZE, PaneId,
    PaneNode, SplitAxis, TabId, TabState,
};
pub use ids::WorkspaceId;
pub use pane_content::{CloseOutcome, PaneContent, ParkOutcome};
pub use pane_tree::{PaneLeaf, PaneTab, PaneTree};
pub use route::FileRoute;
pub use session::{DEFAULT_SIDEBAR_WIDTH, Point, SessionState, clamp_sidebar};
pub use split::{SplitTree, flatten_split, move_boundary, normalize_sizes};
pub use tab_label::{clean_terminal_title, terminal_tab_label};
pub use tasks::{
    Action, Background, EditorInfo, Group, GroupKind, InputDef, InputKind, Orchestrator, Os,
    Outcome, Panel, PickOption, Presentation, Quoted, Quoting, Readiness, ResolvedTask, Reveal,
    ShellOptions, TaskDef, TaskKind, TaskOptions, TaskSet, VarContext, expand, find_tasks_json,
    folder_open_closure, folder_open_hash, input_refs, parse_task_set, preview_line,
    required_inputs, resolve_task, workspace_for_tasks_json,
};
pub use workspace::{Active, CURRENT_VERSION, Registry, WorkspaceRec};

#[cfg(test)]
mod tests;
