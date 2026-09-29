use std::collections::HashMap;

use gpui::{AnyWindowHandle, Subscription, Task};
use xenon_core::WorkspaceId;
use xenon_ide::IdeServer;

use super::git_dirt::DirtMsg;
use super::remote::RemoteRuntime;
use crate::git_dirt::GitDirt;

/// Long-lived integrations and background work owned by the application.
#[derive(Default)]
pub(super) struct AppServices {
    pub ide: Option<IdeServer>,
    pub ide_task: Option<Task<()>>,
    /// Phone remote while on (server, sessions, devices).
    pub remote: Option<RemoteRuntime>,
    /// The shell window; remote work that needs a `Window` runs against it.
    pub main_window: Option<AnyWindowHandle>,
    pub git_dirt: HashMap<WorkspaceId, GitDirt>,
    pub git_dirt_tx: Option<async_channel::Sender<DirtMsg>>,
    pub git_dirt_task: Option<Task<()>>,
    pub bounds_save_task: Option<Task<()>>,
    pub bounds_save_generation: u64,
    pub window_bounds_subscription: Option<Subscription>,
    pub focus_guard: Vec<Subscription>,
}
