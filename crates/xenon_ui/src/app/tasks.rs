//! Run Task palette (⌘⇧R) and Run Build Task (⌘⇧B): picker rows from the
//! workspace's tasks.json, then `start_task`.

use super::task_runs::{Origin, RunRequest};
use super::*;
use crate::task_picker::{TaskPickerEvent, TaskPickerView};
use xenon_core::tasks::{BuildChoice, NO_TASKS, choose_build_task, task_rows};

impl XenonApp {
    pub(super) fn open_task_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ws) = self.active else {
            self.show_toast(super::toasts::no_workspace(), cx);
            return;
        };
        self.dismiss_palettes(cx);
        self.deferred.restore_pane = Some(self.current_focus_owner(window, cx));
        let (rows, error) = match self.load_tasks(ws) {
            Ok(loaded) => {
                let rows = task_rows(&loaded.set, &loaded.folder);
                let error = rows.is_empty().then(|| NO_TASKS.to_string());
                (rows, error)
            }
            Err(e) => (Vec::new(), Some(e)),
        };
        let picker = cx.new(|cx| TaskPickerView::new(rows, error, cx));
        self._task_picker_sub = Some(cx.subscribe(&picker, Self::on_task_picker_event));
        self.task_picker = Some(picker);
        cx.notify();
    }

    /// ⌘⇧B: the default build task; else the only build task; else the picker.
    pub(super) fn run_build_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ws) = self.active else {
            self.show_toast(super::toasts::no_workspace(), cx);
            return;
        };
        let loaded = match self.load_tasks(ws) {
            Ok(loaded) => loaded,
            Err(e) => {
                self.show_toast(super::toasts::failed(xenon_core::tasks::LOAD_FAILED, e), cx);
                return;
            }
        };
        match choose_build_task(&loaded.set) {
            BuildChoice::Run(label) => {
                self.deferred.restore_pane = Some(self.current_focus_owner(window, cx));
                self.run_task_checked(ws, label, false, cx);
            }
            BuildChoice::None => {
                let (title, detail) = xenon_core::tasks::NO_BUILD_TASK;
                self.show_toast(super::toasts::failed(title, detail), cx);
            }
            BuildChoice::Pick => self.open_task_picker(window, cx),
        }
    }

    fn on_task_picker_event(
        &mut self,
        _picker: Entity<TaskPickerView>,
        event: &TaskPickerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TaskPickerEvent::Run(label) | TaskPickerEvent::RunInNewTab(label) => {
                let new_tab = matches!(event, TaskPickerEvent::RunInNewTab(_));
                self.task_picker = None;
                if let Some(ws) = self.active {
                    self.run_task_checked(ws, label.clone(), new_tab, cx);
                }
            }
            TaskPickerEvent::Unsupported(why) => {
                self.show_toast(super::toasts::failed(xenon_core::tasks::CANT_RUN, why), cx);
            }
            TaskPickerEvent::Dismissed => {
                self.task_picker = None;
                self.deferred.pending_focus = self
                    .deferred
                    .restore_pane
                    .take()
                    .or_else(|| Some(self.fallback_content_pane()));
                cx.notify();
            }
        }
    }

    /// Start a task from the Mac, toasting a failure to even start.
    pub(super) fn run_task_checked(
        &mut self,
        workspace: WorkspaceId,
        label: String,
        new_tab: bool,
        cx: &mut Context<Self>,
    ) {
        let request = RunRequest {
            workspace,
            label,
            new_tab,
            origin: Origin::Mac,
        };
        if let Err(error) = self.start_task(request, cx) {
            self.show_toast(
                super::toasts::failed(xenon_core::tasks::COULDNT_RUN, error),
                cx,
            );
        }
        cx.notify();
    }
}

#[cfg(feature = "visual-tests")]
impl XenonApp {
    pub(crate) fn visual_task_input(
        &mut self,
        scene: super::visual::Scene,
        cx: &mut Context<Self>,
    ) {
        use super::visual::Scene;
        use crate::task_input::{QuestionKind, TaskQuestion};
        let question = match scene {
            Scene::TaskInputPassword => TaskQuestion {
                id: "token".into(),
                description: "Deploy token".into(),
                default: None,
                kind: QuestionKind::Text { password: true },
            },
            Scene::TaskInputPick => TaskQuestion {
                id: "env".into(),
                description: "Which environment?".into(),
                default: Some("staging".into()),
                kind: QuestionKind::Pick {
                    options: ["development", "staging", "production"]
                        .into_iter()
                        .map(|v| xenon_core::PickOption {
                            label: v.into(),
                            value: v.into(),
                        })
                        .chain([xenon_core::PickOption {
                            label: "Preview (per branch)".into(),
                            value: "preview".into(),
                        }])
                        .collect(),
                },
            },
            _ => TaskQuestion {
                id: "who".into(),
                description: "Who to greet".into(),
                default: Some("world".into()),
                kind: QuestionKind::Text { password: false },
            },
        };
        self.show_input_prompt(question, cx);
        if scene == Scene::TaskInputPassword
            && let Some((view, _)) = &self.services.tasks.input
        {
            view.update(cx, |view, cx| view.visual_type("s3cret-value", cx));
        }
    }

    /// Run a fixture task, answering its one input.
    pub(crate) fn visual_run_task(&mut self, label: &str, answer: &str, cx: &mut Context<Self>) {
        let Some(ws) = self.active else { return };
        self.run_task_checked(ws, label.to_string(), false, cx);
        if let Some(token) = self.services.tasks.pending.as_ref().map(|p| p.token) {
            self.answer_task_input(token, None, Some(answer.to_string()), cx);
        }
    }
}
