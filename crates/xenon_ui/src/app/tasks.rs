//! Run Task palette: load `.vscode/tasks.json` shell tasks and inject into a
//! workspace terminal (current tab or a newly opened one).

use super::*;
use crate::task_picker::{TaskPickerEvent, TaskPickerView};
use xenon_core::{ShellTask, load_shell_tasks};

impl XenonApp {
    pub(super) fn open_task_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.active.and_then(|id| self.workspace_root(id)) else {
            return;
        };
        self.finder = None;
        self.workspace_picker = None;
        self.command_palette = None;
        self.deferred.restore_pane = self.focused_pane(window, cx);
        let (tasks, error) = match load_shell_tasks(&root) {
            Ok(tasks) if tasks.is_empty() => (
                Vec::new(),
                Some("No shell tasks in .vscode/tasks.json".into()),
            ),
            Ok(tasks) => (tasks, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let picker = cx.new(|cx| TaskPickerView::new(tasks, error, cx));
        self._task_picker_sub = Some(cx.subscribe(&picker, Self::on_task_picker_event));
        self.task_picker = Some(picker);
        cx.notify();
    }

    fn on_task_picker_event(
        &mut self,
        _picker: Entity<TaskPickerView>,
        event: &TaskPickerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TaskPickerEvent::Run(task) => {
                self.task_picker = None;
                self.run_task(task.clone(), false, cx);
            }
            TaskPickerEvent::RunInNew(task) => {
                self.task_picker = None;
                self.run_task(task.clone(), true, cx);
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

    fn run_task(&mut self, task: ShellTask, new_terminal: bool, cx: &mut Context<Self>) {
        if new_terminal || self.active_terminal().is_none() {
            self.add_terminal(cx);
        }
        let mut line = task.inject_line();
        // One-shot tab: exit the shell when the command finishes so terminal
        // auto-close (same rules as shell exit) can apply.
        if new_terminal {
            let body = line.trim_end_matches('\n');
            line = format!("{body}; exit\n");
        }
        let Some(terminal) = self.active_terminal() else {
            return;
        };
        terminal.update(cx, |term, cx| {
            term.inject_text(&line, cx);
        });
        self.deferred.pending_focus = Some(FocusPane::Terminal);
        cx.notify();
    }
}
