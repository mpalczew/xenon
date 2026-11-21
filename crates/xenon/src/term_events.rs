//! The terminal service's side of the window: what its events change here, and how
//! tabs find, create, adopt and end their service terminals.

use std::path::Path;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use xenon_core::{TabId, TermId, WorkspaceId};
use xenon_term::{Event, Spawn, TermStatus};

use crate::app::App;
use crate::model::Tab;

impl App {
    /// Remember what the service holds: ids, workspaces and process ids.
    pub(crate) fn learn_terms(&self) {
        match self.svc.list() {
            Ok(list) => {
                let mut known = self.known.borrow_mut();
                known.clear();
                known.extend(list.iter().map(|t| (t.id, (t.workspace, t.pid))));
            }
            Err(error) => eprintln!("[xg] terminal list: {error:#}"),
        }
    }

    /// Drain the service's events on the main loop.
    pub(crate) fn listen_service(self: &Rc<Self>) {
        let rx = self.svc.events();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                let Some(app) = weak.upgrade() else { break };
                app.service_event(event);
            }
        });
    }

    fn service_event(self: &Rc<Self>, event: Event) {
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
            Event::Created { id, workspace } => {
                self.learn_terms();
                self.known.borrow_mut().entry(id).or_insert((workspace, 0));
                self.refresh_pids();
                self.adopt_orphans(workspace);
            }
            Event::Closed { id } => {
                self.known.borrow_mut().remove(&id);
                let tab = self.model.borrow().tab_of_term(id);
                if let Some((ws, tab)) = tab {
                    self.close_tab_ui(ws, tab);
                }
            }
            Event::Phone => self.remote_publish(),
            Event::Lost => self.toast("Lost the terminal service. Quit and start Xenon again."),
        }
    }

    fn refresh_pids(&self) {
        let known = self.known.borrow();
        for view in self.terms.borrow().values() {
            let pid = known.get(&view.id).map(|(_, pid)| *pid);
            view.pid.set(pid.filter(|p| *p > 0));
        }
    }

    fn view_of(&self, id: TermId) -> Option<Rc<crate::term::TermView>> {
        self.terms.borrow().values().find(|v| v.id == id).cloned()
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

    /// The tab on screen was looked at: its attention clears here and in the service.
    pub(crate) fn dismiss_viewed(&self) -> bool {
        let seen = self.model.borrow_mut().dismiss_viewed();
        if let Some(id) = seen {
            self.svc.seen(id);
        }
        seen.is_some()
    }

    /// The adoption rule: any live terminal of `ws` that no tab references gets a new,
    /// unselected tab. It covers terminals the phone made and ones a crashed window left.
    pub(crate) fn adopt_orphans(self: &Rc<Self>, ws: WorkspaceId) {
        let orphans: Vec<TermId> = self
            .known
            .borrow()
            .iter()
            .filter(|(_, (w, _))| *w == ws)
            .map(|(id, _)| *id)
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

    /// The body of a terminal tab: its view, over the tab's service terminal, which is
    /// made fresh when the tab has none (a restarted service, or a new tab).
    pub(crate) fn terminal_body(
        self: &Rc<Self>,
        ws: WorkspaceId,
        tab: &Tab,
        cwd: &Path,
    ) -> Option<gtk::Widget> {
        let live = tab.term.filter(|id| self.known.borrow().contains_key(id));
        let id = match live {
            Some(id) => id,
            None => {
                let spawn = self
                    .tasks
                    .take_spawn(ws, tab.id)
                    .unwrap_or_else(|| Spawn::shell(cwd));
                match self.svc.create(ws, spawn) {
                    Ok(id) => id,
                    Err(error) => {
                        self.toast(&format!(
                            "Couldn\u{2019}t start a terminal \u{2014} {error}"
                        ));
                        return None;
                    }
                }
            }
        };
        if live.is_none() {
            self.known.borrow_mut().insert(id, (ws, 0));
            if let Some(t) = self.model.borrow_mut().tab_mut(ws, tab.id) {
                t.term = Some(id);
            }
            self.save_session(ws);
        }
        let view = crate::term::TermView::new(self, ws, tab.id, id, cwd);
        view.pid.set(
            self.known
                .borrow()
                .get(&id)
                .map(|(_, pid)| *pid)
                .filter(|p| *p > 0),
        );
        self.restyle_term(&view);
        let root = view.root.clone().upcast();
        self.terms.borrow_mut().insert((ws, tab.id), view);
        Some(root)
    }

    /// The tab is gone: hang up its terminal.
    pub(crate) fn end_term(&self, key: (WorkspaceId, TabId)) {
        if let Some(view) = self.terms.borrow_mut().remove(&key) {
            self.known.borrow_mut().remove(&view.id);
            self.svc.close(view.id);
        }
    }
}
