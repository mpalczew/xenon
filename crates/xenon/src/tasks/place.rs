use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use xenon_core::tasks as shared;
use xenon_core::{Panel, ResolvedTask, Reveal, TabId, WorkspaceId};

use super::{Placement, Spawn};
use crate::app::App;

impl App {
    pub(super) fn place_task_terminal(
        self: &Rc<Self>,
        task: &ResolvedTask,
        ws: WorkspaceId,
        placement: Placement,
        spawn: Spawn,
    ) -> Option<TabId> {
        let Placement { force_new, quiet } = placement;
        let pres = task.presentation;
        let key = (ws, task.label.clone());
        let reusable = if force_new {
            None
        } else {
            match pres.panel {
                Panel::New => None,
                Panel::Shared => self.tasks.shared.borrow().get(&ws).copied(),
                Panel::Dedicated => self.tasks.dedicated.borrow().get(&key).copied(),
            }
            .filter(|tab| {
                self.model
                    .borrow()
                    .tab(ws, *tab)
                    .is_some_and(|t| t.exited.is_some())
            })
        };
        let select = !quiet && (force_new || pres.reveal == Reveal::Always);
        let before = gtk::prelude::GtkWindowExt::focus(&self.win);
        let tab = match reusable {
            Some(tab) => {
                self.bodies.borrow_mut().remove(&(ws, tab));
                self.terms.borrow_mut().remove(&(ws, tab));
                self.remote_tab_gone(ws, tab);
                let mut model = self.model.borrow_mut();
                model.recycle_terminal(ws, tab, &task.label);
                if select {
                    model.select_tab(ws, tab);
                }
                tab
            }
            None => self
                .model
                .borrow_mut()
                .new_task_terminal(ws, &task.label, select)?,
        };
        self.tasks.spawns.borrow_mut().insert((ws, tab), spawn);
        match pres.panel {
            Panel::Shared => {
                self.tasks.shared.borrow_mut().insert(ws, tab);
            }
            Panel::Dedicated => {
                self.tasks.dedicated.borrow_mut().insert(key, tab);
            }
            Panel::New => {}
        }
        self.sync(ws);
        self.save_session(ws);
        if select && (pres.focus || force_new) {
            self.focus_terminal();
        } else if select {
            let weak = Rc::downgrade(self);
            glib::idle_add_local_once(move || match before.filter(|w| w.is_mapped()) {
                Some(widget) => {
                    widget.grab_focus();
                }
                None => {
                    if let Some(app) = weak.upgrade() {
                        app.focus_terminal();
                    }
                }
            });
        }
        Some(tab)
    }

    pub(crate) fn task_finished(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let Some((run_id, label)) = self.tasks.owners.borrow().get(&(ws, tab)).cloned() else {
            return;
        };
        let code = self
            .terms
            .borrow()
            .get(&(ws, tab))
            .and_then(|view| view.status.get())
            .and_then(|status| (status & 0x7f == 0).then_some((status >> 8) & 0xff));
        let Some((pres, background)) = self.with_run(run_id, |run| {
            let launched = run.launched.get_mut(&label);
            let background = launched.as_ref().is_some_and(|l| l.background);
            if let Some(l) = launched {
                l.done = true;
            }
            let pres = run.plan.set.get(&label).map(|d| d.presentation);
            (pres.unwrap_or_default(), background)
        }) else {
            return;
        };
        if code == Some(0) {
            if pres.close {
                self.close_tab_ui(ws, tab);
            }
            let actions = self
                .with_run(run_id, |r| r.plan.orch.satisfied(&label))
                .unwrap_or_default();
            self.apply_actions(run_id, actions);
            return;
        }
        if pres.reveal == Reveal::Silent {
            self.model.borrow_mut().select_tab(ws, tab);
            self.sync(ws);
        }
        if !background {
            self.fail_run(run_id, &label, &shared::exit_reason(code));
        }
    }

    pub(crate) fn task_tab_gone(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        self.tasks.spawns.borrow_mut().remove(&(ws, tab));
        let owner = self.tasks.owners.borrow_mut().remove(&(ws, tab));
        let Some((run_id, label)) = owner else { return };
        let unfinished = self
            .with_run(run_id, |r| {
                r.launched
                    .get(&label)
                    .is_some_and(|l| !l.done && !l.background)
            })
            .unwrap_or(false);
        if unfinished {
            self.fail_run(run_id, &label, &shared::exit_reason(None));
        }
    }
}
