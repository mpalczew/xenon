use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::{Subscription, Task};
use xenon_core::WorkspaceId;
use xenon_ide::IdeServer;
use xenon_remote::RemoteServer;

use super::git_dirt::DirtMsg;
use crate::git_dirt::GitDirt;

/// Long-lived integrations and background work owned by the application.
#[derive(Default)]
pub(super) struct AppServices {
    pub ide: Option<IdeServer>,
    pub ide_task: Option<Task<()>>,
    pub remote: Option<RemoteServer>,
    pub remote_task: Option<Task<()>>,
    pub remote_frame_seq: Arc<Mutex<HashMap<(WorkspaceId, u64), u64>>>,
    pub git_dirt: HashMap<WorkspaceId, GitDirt>,
    pub git_dirt_tx: Option<async_channel::Sender<DirtMsg>>,
    pub git_dirt_task: Option<Task<()>>,
    pub bounds_save_task: Option<Task<()>>,
    pub bounds_save_generation: u64,
    pub window_bounds_subscription: Option<Subscription>,
}
