//! Consent for `runOptions.runOn: "folderOpen"` tasks. Nothing runs
//! automatically until the user allows it for this workspace; the approval is
//! keyed to a hash of the tasks it covers, so an edited tasks.json asks again.
//! The prompt is a non-modal card: it never takes focus or Escape. Its two
//! shortcuts work from anywhere while it shows.

use super::task_runs::{Origin, RunRequest};
use super::tasks::{row_command, with_preview_ctx};
use super::*;
use xenon_core::{TaskSet, folder_open_closure, folder_open_hash};
use xenon_design_system::{ActionButton, TypeRole, Typography, action_button, shortcut_text};
use xenon_store::TaskTrust;

pub(super) struct TrustPrompt {
    workspace: WorkspaceId,
    folder: PathBuf,
    hash: String,
    /// Exactly what an approval covers: label, command line, why it is listed.
    lines: Vec<(String, String, &'static str)>,
}

impl XenonApp {
    /// Once per workspace per session, when it opens.
    pub(super) fn check_folder_open_tasks(&mut self, ws: WorkspaceId, cx: &mut Context<Self>) {
        if !self.services.tasks.folder_checked.insert(ws) {
            return;
        }
        let Ok(loaded) = self.load_tasks(ws) else {
            return;
        };
        let Some(hash) = folder_open_hash(&loaded.set) else {
            return;
        };
        match xenon_store::task_trust(&loaded.folder, &hash) {
            TaskTrust::Allowed => self.run_folder_open(ws, &loaded.set, cx),
            TaskTrust::Denied => {}
            TaskTrust::Unknown => self.show_trust_prompt(ws, &loaded, hash, cx),
        }
    }

    fn run_folder_open(&mut self, ws: WorkspaceId, set: &TaskSet, cx: &mut Context<Self>) {
        let labels: Vec<String> = set
            .tasks
            .iter()
            .filter(|t| t.run_on_folder_open && t.unsupported.is_none())
            .map(|t| t.label.clone())
            .collect();
        for label in labels {
            let request = RunRequest {
                workspace: ws,
                label,
                new_tab: false,
                origin: Origin::Auto,
            };
            if let Err(error) = self.start_task(request, cx) {
                self.show_toast(
                    super::toasts::failed("Couldn’t run task on open", error),
                    cx,
                );
            }
        }
    }

    fn show_trust_prompt(
        &mut self,
        ws: WorkspaceId,
        loaded: &super::task_runs::LoadedTasks,
        hash: String,
        cx: &mut Context<Self>,
    ) {
        let (set, folder) = (&*loaded.set, loaded.folder.as_path());
        let lines = with_preview_ctx(set, folder, |ctx| {
            folder_open_closure(set)
                .into_iter()
                .map(|t| {
                    let why = if t.run_on_folder_open {
                        "runs on open"
                    } else {
                        "dependency"
                    };
                    (t.label.clone(), row_command(t, ctx), why)
                })
                .collect()
        });
        self.services.tasks.trust = Some(TrustPrompt {
            workspace: ws,
            folder: folder.to_path_buf(),
            hash,
            lines,
        });
        cx.notify();
    }

    /// Allow / Don't allow. Without a showing prompt the key passes through.
    pub(super) fn answer_folder_tasks(&mut self, allow: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.services.tasks.trust.take() else {
            cx.propagate();
            return;
        };
        // The file may have changed since the card was drawn: approve only
        // what is on disk now if it is what the user saw.
        let current = self.load_tasks(prompt.workspace).ok();
        let still_same = current
            .as_ref()
            .and_then(|l| folder_open_hash(&l.set))
            .is_some_and(|h| h == prompt.hash);
        if !still_same {
            if let Some(loaded) = current
                && let Some(hash) = folder_open_hash(&loaded.set)
            {
                self.show_trust_prompt(prompt.workspace, &loaded, hash, cx);
            }
            return;
        }
        if let Err(error) = xenon_store::set_task_trust(&prompt.folder, &prompt.hash, allow) {
            self.show_toast(
                super::toasts::failed("Couldn’t save that choice", error),
                cx,
            );
        }
        if allow && let Some(loaded) = current {
            self.run_folder_open(prompt.workspace, &loaded.set, cx);
        }
        cx.notify();
    }

    #[cfg(feature = "visual-tests")]
    pub(crate) fn show_trust_prompt_fixture(&mut self, cx: &mut Context<Self>) {
        self.services.tasks.trust = Some(TrustPrompt {
            workspace: self.active.unwrap_or_else(WorkspaceId::new),
            folder: PathBuf::from("/Users/me/src/app"),
            hash: "fixture".into(),
            lines: vec![
                (
                    "dev server".into(),
                    "npm run dev  (after install)".into(),
                    "runs on open",
                ),
                ("install".into(), "npm ci".into(), "dependency"),
            ],
        });
        cx.notify();
    }

    pub(crate) fn render_trust_prompt(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let prompt = self.services.tasks.trust.as_ref()?;
        let colors = cx.theme().colors().clone();
        let name = prompt
            .folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut list = div()
            .id("task-trust-list")
            .max_h(px(220.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (label, command, why) in &prompt.lines {
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(colors.background)
                    .border_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .type_role(TypeRole::ControlLabel, cx)
                            .text_color(colors.text_muted)
                            .child(format!("{label} · {why}")),
                    )
                    .child(
                        div()
                            .type_role(TypeRole::Code, cx)
                            .child(format!("$ {command}")),
                    ),
            );
        }
        let allow = cx.listener(|this, _, _, cx| this.answer_folder_tasks(true, cx));
        let deny = cx.listener(|this, _, _, cx| this.answer_folder_tasks(false, cx));
        let card = div()
            .id("task-trust-card")
            .occlude()
            .key_context("TaskTrust")
            .absolute()
            .bottom(px(16.))
            .right(px(16.))
            .w(px(460.))
            .p_3()
            .rounded_lg()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .type_role(TypeRole::Body, cx)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(format!("Run tasks automatically in {name}?")),
            )
            .child(
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child("This folder’s tasks.json asks to run these commands every time it opens. They run as you."),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(action_button(
                        "Don’t allow",
                        ActionButton::secondary("Don’t allow"),
                        cx,
                        deny,
                    ))
                    .child(action_button("Allow", ActionButton::primary("Allow"), cx, allow)),
            )
            .child(
                div()
                    .type_role(TypeRole::ControlLabel, cx)
                    .text_color(colors.text_muted)
                    .child(shortcut_text("⌘⌥↩ allow  ·  ⌘⌥⌫ don’t allow  ·  asks again if tasks.json changes")),
            );
        Some(card.into_any_element())
    }
}
