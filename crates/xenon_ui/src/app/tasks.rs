//! Run Task palette (⌘⇧R) and Run Build Task (⌘⇧B): picker rows from the
//! workspace's tasks.json, then `start_task`.

use super::task_runs::{Origin, RunRequest};
use super::*;
use crate::task_picker::{TaskPickerEvent, TaskPickerView, TaskRow};
use xenon_core::{EditorInfo, GroupKind, TaskDef, TaskSet, VarContext};

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
                let error = rows
                    .is_empty()
                    .then(|| "No tasks in .vscode/tasks.json".to_string());
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
                self.show_toast(super::toasts::failed("Couldn’t load tasks", e), cx);
                return;
            }
        };
        let builds: Vec<&TaskDef> = loaded
            .set
            .tasks
            .iter()
            .filter(|t| t.group.is_some_and(|g| g.kind == GroupKind::Build))
            .collect();
        let chosen = loaded
            .set
            .default_of(GroupKind::Build)
            .or(if builds.len() == 1 {
                builds.first().copied()
            } else {
                None
            })
            .map(|t| t.label.clone());
        match chosen {
            Some(label) => {
                self.deferred.restore_pane = Some(self.current_focus_owner(window, cx));
                self.run_task_checked(ws, label, false, cx);
            }
            None if builds.is_empty() => self.show_toast(
                super::toasts::failed("No build task", "Mark one with \"group\": \"build\"."),
                cx,
            ),
            None => self.open_task_picker(window, cx),
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
                self.show_toast(super::toasts::failed("Can’t run that task", why), cx);
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
            self.show_toast(super::toasts::failed("Couldn’t run task", error), cx);
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

/// Run `f` with a variable context that knows only the workspace (no editor,
/// no inputs): what the picker and the trust prompt can show before a run.
pub(super) fn with_preview_ctx<R>(
    set: &TaskSet,
    folder: &Path,
    f: impl FnOnce(&VarContext) -> R,
) -> R {
    let env = |k: &str| std::env::var(k).ok();
    let empty = std::collections::BTreeMap::new();
    let secrets = std::collections::BTreeSet::new();
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let editor: Option<&EditorInfo> = None;
    f(&VarContext {
        workspace: folder,
        editor,
        env: &env,
        config: &empty,
        inputs: &empty,
        secret_inputs: &secrets,
        default_build_task: set.default_of(GroupKind::Build).map(|t| t.label.as_str()),
        exec_path: "",
        user_home: &home,
    })
}

/// Picker rows: every non-hidden task, unsupported ones included.
pub(crate) fn task_rows(set: &TaskSet, folder: &Path) -> Vec<TaskRow> {
    let mut rows: Vec<TaskRow> = with_preview_ctx(set, folder, |ctx| {
        set.tasks
            .iter()
            .filter(|t| !t.hide)
            .map(|t| TaskRow {
                label: t.label.clone(),
                command_line: row_command(t, ctx),
                detail: t.detail.clone(),
                tag: group_tag(t),
                unsupported: t.unsupported.clone(),
            })
            .collect()
    });
    rows.extend(set.warnings.iter().map(|w| TaskRow {
        label: "Skipped".into(),
        command_line: String::new(),
        detail: None,
        tag: None,
        unsupported: Some(w.clone()),
    }));
    rows
}

pub(super) fn row_command(task: &TaskDef, ctx: &VarContext) -> String {
    let line = xenon_core::preview_line(task, ctx);
    if task.depends_on.is_empty() {
        return line;
    }
    let deps = task.depends_on.join(", ");
    if line.is_empty() {
        format!("(runs {deps})")
    } else {
        format!("{line}  (after {deps})")
    }
}

fn group_tag(task: &TaskDef) -> Option<String> {
    let group = task.group?;
    let kind = match group.kind {
        GroupKind::Build => "build",
        GroupKind::Test => "test",
    };
    Some(if group.is_default {
        format!("{kind} · default")
    } else {
        kind.to_string()
    })
}
