//! Asking for a run's `inputs`: one question at a time, on the Mac as a modal
//! or on the phone that started the run.

use super::task_runs::Origin;
use super::*;
use crate::task_input::{QuestionKind, TaskInputEvent, TaskInputView, TaskQuestion};
use xenon_core::InputKind;

impl XenonApp {
    /// Ask the first unanswered input, or launch when none remain.
    pub(super) fn ask_next_input(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.services.tasks.pending.as_ref() else {
            return;
        };
        let Some(id) = pending.ids.first().cloned() else {
            let Some(pending) = self.services.tasks.pending.take() else {
                return;
            };
            self.services.tasks.input = None;
            self.launch_plan(pending.plan, cx);
            return;
        };
        let token = pending.token;
        let Some(def) = pending.plan.loaded.inputs.get(&id) else {
            return;
        };
        let question = TaskQuestion {
            id: def.id.clone(),
            description: def.description.clone(),
            default: def.default.clone(),
            kind: match &def.kind {
                InputKind::PickString { options } => QuestionKind::Pick {
                    options: options.clone(),
                },
                InputKind::PromptString { password } => QuestionKind::Text {
                    password: *password,
                },
                InputKind::Command => return,
            },
        };
        match pending.plan.origin.clone() {
            Origin::Remote(remote) => (remote.ask)(token, question),
            Origin::Mac | Origin::Auto => self.show_input_prompt(question, cx),
        }
    }

    pub(super) fn show_input_prompt(&mut self, question: TaskQuestion, cx: &mut Context<Self>) {
        self.dismiss_palettes(cx);
        let view = cx.new(|cx| TaskInputView::new(question, cx));
        let sub = cx.subscribe(&view, |this, _, event, cx| match event {
            TaskInputEvent::Answered(value) => {
                let token = this.services.tasks.pending.as_ref().map(|p| p.token);
                if let Some(token) = token {
                    this.answer_task_input(token, None, Some(value.clone()), cx);
                }
            }
            TaskInputEvent::Cancelled => {
                let token = this.services.tasks.pending.as_ref().map(|p| p.token);
                if let Some(token) = token {
                    this.answer_task_input(token, None, None, cx);
                }
            }
        });
        self.services.tasks.input = Some((view, sub));
        cx.notify();
    }

    /// An answer (`Some`) or a cancel (`None`) for the pending question.
    /// A stale token (the run was replaced) is ignored, and so is an answer
    /// from anyone but the surface that was asked: `from` is the phone
    /// connection, `None` for the Mac.
    pub(crate) fn answer_task_input(
        &mut self,
        token: u64,
        from: Option<u64>,
        value: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.services.tasks.pending.as_mut() else {
            return;
        };
        let asked = match &pending.plan.origin {
            Origin::Remote(remote) => Some(remote.conn),
            Origin::Mac | Origin::Auto => None,
        };
        if pending.token != token || asked != from {
            return;
        }
        let mac = !matches!(pending.plan.origin, Origin::Remote(_));
        match value {
            Some(value) if !pending.ids.is_empty() => {
                let id = pending.ids.remove(0);
                pending.plan.inputs.insert(id, value);
                self.ask_next_input(cx);
            }
            _ => {
                self.services.tasks.pending = None;
                self.services.tasks.input = None;
            }
        }
        if mac {
            self.services.tasks.input = self
                .services
                .tasks
                .input
                .take()
                .filter(|_| self.services.tasks.pending.is_some());
            self.deferred.pending_focus = self
                .deferred
                .restore_pane
                .take()
                .or(Some(FocusOwner::Terminal));
        }
        cx.notify();
    }
}
