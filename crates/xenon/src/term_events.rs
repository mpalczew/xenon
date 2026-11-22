use std::path::Path;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use xenon_core::{TabId, TermId, WorkspaceId};
use xenon_term::{Event, Spawn, TermStatus};

use crate::app::App;
use crate::model::Tab;
use crate::term::TermView;

impl App {
    pub(crate) fn listen_holders(self: &Rc<Self>) {
        let rx = self.holders.events();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                let Some(app) = weak.upgrade() else { break };
                app.holder_event(event);
            }
        });
    }

    fn holder_event(self: &Rc<Self>, event: Event) {
        match event {
            Event::Output { id, bytes } => {
                if let Some(view) = self.view_of(id) {
                    view.feed(&bytes);
                }
            }
            Event::Status { id, status } => self.term_status(id, status),
            Event::Size { id, size } => {
                if let Some(view) = self.view_of(id) {
                    view.follow(size);
                }
            }
            Event::Ready { id, info } => {
                if let Some(view) = self.view_of(id) {
                    view.pid.set(Some(info.pid).filter(|p| *p > 0));
                }
                self.adopt_orphans(info.workspace);
                self.refresh_builds();
            }
            Event::Closed { id } => {
                let tab = self.model.borrow().tab_of_term(id);
                if let Some((ws, tab)) = tab {
                    self.close_tab_ui(ws, tab);
                }
                self.refresh_builds();
            }
            Event::Failed { id, error } => {
                self.toast(&format!(
                    "Couldn\u{2019}t start a terminal \u{2014} {error}"
                ));
                let tab = self.model.borrow().tab_of_term(id);
                if let Some((ws, tab)) = tab {
                    self.close_tab_ui(ws, tab);
                }
            }
            Event::Builds => self.refresh_builds(),
        }
    }

    fn view_of(&self, id: TermId) -> Option<Rc<TermView>> {
        self.terms.borrow().get(&id).cloned()
    }

    fn term_status(self: &Rc<Self>, id: TermId, status: TermStatus) {
        let Some((ws, tab)) = self.model.borrow().tab_of_term(id) else {
            return;
        };
        let applied = self.model.borrow_mut().apply_status(ws, tab, status);
        let dismissed = self.dismiss_viewed();
        if applied.relabel {
            self.relabel(ws, tab);
        }
        if let Some(code) = applied.exited {
            self.term_exited(ws, tab, code);
        }
        if applied.dots || dismissed {
            self.paint_all();
        }
    }

    pub(crate) fn dismiss_viewed(&self) -> bool {
        let seen = self.model.borrow_mut().dismiss_viewed();
        if let Some(id) = seen {
            self.holders.seen(id);
        }
        seen.is_some()
    }

    pub(crate) fn adopt_orphans(self: &Rc<Self>, ws: WorkspaceId) {
        let orphans: Vec<TermId> = self
            .holders
            .list()
            .into_iter()
            .filter(|t| t.workspace == ws)
            .map(|t| t.id)
            .collect();
        let mut changed = false;
        for id in orphans {
            let mut model = self.model.borrow_mut();
            let live = model.spaces.get(&ws).is_some_and(|s| s.live);
            if live && model.tab_of_term(id).is_none() {
                changed |= model.adopt_terminal(ws, id).is_some();
            }
        }
        if changed {
            self.sync(ws);
            self.save_session(ws);
        }
    }

    pub(crate) fn terminal_body(
        self: &Rc<Self>,
        ws: WorkspaceId,
        tab: &Tab,
        cwd: &Path,
    ) -> (gtk::Widget, TermId) {
        let live = tab.term.and_then(|id| self.holders.info(id));
        let id = match &live {
            Some(info) => info.id,
            None => {
                let spawn = self
                    .tasks
                    .take_spawn(ws, tab.id)
                    .unwrap_or_else(|| Spawn::shell(cwd));
                self.holders.open(ws, spawn)
            }
        };
        if let Some(info) = live.as_ref().filter(|info| !info.build.speaks) {
            if let Some(t) = self.model.borrow_mut().tab_mut(ws, tab.id) {
                t.term = Some(id);
            }
            return (self.unreadable_notice(info), id);
        }
        if live.is_none() {
            if let Some(t) = self.model.borrow_mut().tab_mut(ws, tab.id) {
                t.term = Some(id);
            }
            self.save_session(ws);
        }
        let view = TermView::new(self, ws, tab.id, id, cwd);
        view.pid.set(live.map(|info| info.pid).filter(|p| *p > 0));
        self.restyle_term(&view);
        let root = view.root.clone().upcast();
        self.terms.borrow_mut().insert(id, view);
        (root, id)
    }

    pub(crate) fn term_view(&self, ws: WorkspaceId, tab: TabId) -> Option<Rc<TermView>> {
        let id = self.bodies.borrow().get(&(ws, tab))?.term?;
        self.terms.borrow().get(&id).cloned()
    }

    pub(crate) fn end_term(&self, key: (WorkspaceId, TabId)) {
        let Some(id) = self.bodies.borrow_mut().remove(&key).and_then(|b| b.term) else {
            return;
        };
        self.terms.borrow_mut().remove(&id);
        self.holders.close(id);
    }
}
