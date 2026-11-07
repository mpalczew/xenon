//! Pure data model for Xenon: workspaces, session state, and VS Code shell-task
//! parsing. No gpui — persistence lives in `xenon_store`.

mod activity;
mod agent;
mod attention;
mod attention_map;
mod clean_output;
mod content;
mod ids;
pub mod outline_text;
mod pane_content;
mod pane_find;
mod pane_moves;
mod pane_new_splits;
mod pane_split;
mod pane_tree;
mod paste;
mod route;
mod session;
mod split;
mod tab_label;
pub mod tasks;
mod workspace;

pub use activity::TerminalActivity;
pub use agent::IDLE_AFTER;
pub use attention::{AttentionReason, WorkspaceDot, workspace_dot, workspace_dot_from_tabs};
pub use attention_map::AttentionMap;
pub use clean_output::{clean_agent_output, extract_code};
pub use content::{ContentLayout, DropEdge, PaneId, PaneNode, SplitAxis, TabId, TabState};
pub use ids::WorkspaceId;
pub use pane_content::{CloseOutcome, PaneContent, ParkOutcome};
pub use pane_tree::{PaneLeaf, PaneTab, PaneTree};
pub use paste::{clipboard_image_path, shell_path, terminal_paths_text};
pub use route::FileRoute;
pub use session::{Point, SessionState};

pub use tab_label::{clean_terminal_title, terminal_tab_label};
pub use tasks::{
    Action, EditorInfo, Orchestrator, Panel, ResolvedTask, Reveal, TaskSet, folder_open_hash,
};
pub use workspace::{Active, Registry, WorkspaceRec};
