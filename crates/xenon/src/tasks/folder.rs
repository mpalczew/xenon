//! folderOpen tasks: run them if the user allowed them for this workspace,
//! else ask with the consent card.

use std::rc::Rc;

use xenon_core::tasks::{self as shared, LoadedTasks};
use xenon_core::{TaskSet, WorkspaceId};
use xenon_store::TaskTrust;

use super::TrustPrompt;
use crate::app::App;

impl App {
    /// Once per workspace per session, when it opens: run its folderOpen
    /// tasks if the user allowed them, else ask.
    pub(crate) fn check_folder_open_tasks(self: &Rc<Self>, ws: WorkspaceId) {
        if !self.tasks.folder_checked.borrow_mut().insert(ws) {
            return;
        }
        let Ok(loaded) = self.load_workspace_tasks(ws) else {
            return;
        };
        let Some(hash) = xenon_core::folder_open_hash(&loaded.set) else {
            return;
        };
        match xenon_store::task_trust(&loaded.folder, &hash) {
            TaskTrust::Allowed => self.run_folder_open(ws, &loaded.set),
            TaskTrust::Denied => {}
            TaskTrust::Unknown => self.show_trust_prompt(ws, &loaded, hash),
        }
    }

    fn run_folder_open(self: &Rc<Self>, ws: WorkspaceId, set: &TaskSet) {
        for label in shared::folder_open_labels(set) {
            if let Err(error) = self.start_task(
                ws,
                label,
                super::Start {
                    quiet: true,
                    ..super::Start::default()
                },
            ) {
                self.toast_failed(shared::COULDNT_RUN_ON_OPEN, &error);
            }
        }
    }

    fn show_trust_prompt(self: &Rc<Self>, ws: WorkspaceId, loaded: &LoadedTasks, hash: String) {
        let lines = shared::trust_lines(&loaded.set, &loaded.folder);
        self.trust_card
            .show(&shared::trust_title(&loaded.folder), &lines);
        *self.tasks.trust.borrow_mut() = Some(TrustPrompt {
            ws,
            folder: loaded.folder.clone(),
            hash,
        });
    }

    /// Allow / Don't allow. Without a showing prompt nothing happens.
    pub(crate) fn answer_folder_tasks(self: &Rc<Self>, allow: bool) {
        let Some(prompt) = self.tasks.trust.borrow_mut().take() else {
            return;
        };
        self.trust_card.hide();
        let current = self.load_workspace_tasks(prompt.ws).ok();
        let still_same = current
            .as_ref()
            .and_then(|l| xenon_core::folder_open_hash(&l.set))
            .is_some_and(|h| h == prompt.hash);
        if !still_same {
            if let Some(loaded) = current
                && let Some(hash) = xenon_core::folder_open_hash(&loaded.set)
            {
                self.show_trust_prompt(prompt.ws, &loaded, hash);
            }
            return;
        }
        if let Err(error) = xenon_store::set_task_trust(&prompt.folder, &prompt.hash, allow) {
            self.toast_failed(shared::COULDNT_SAVE_CHOICE, &error.to_string());
        }
        if allow && let Some(loaded) = current {
            self.run_folder_open(prompt.ws, &loaded.set);
        }
    }
}
