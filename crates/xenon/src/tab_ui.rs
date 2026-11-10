use std::rc::Rc;

use gtk::glib;
use xenon_core::{DropEdge, PaneId, TabId, WorkspaceId};

use crate::app::App;

impl App {
    pub(crate) fn tab_event(
        self: &Rc<Self>,
        ws: WorkspaceId,
        child: gtk::Widget,
        then: impl FnOnce(&Rc<App>, WorkspaceId, TabId) + 'static,
    ) {
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(app) = weak.upgrade() else { return };
            if let Some(tab) = app.tab_of(ws, &child) {
                then(&app, ws, tab);
            }
        });
    }

    pub(crate) fn tab_of(&self, ws: WorkspaceId, child: &gtk::Widget) -> Option<TabId> {
        self.bodies
            .borrow()
            .iter()
            .find(|((w, _), body)| *w == ws && *body == child)
            .map(|((_, t), _)| *t)
    }

    pub(crate) fn tab_transfer_done(self: &Rc<Self>, ws: WorkspaceId) {
        self.transfer_queued.set(false);
        if self.off_bar.take() {
            self.changed(ws);
        } else if let Some((tab, pane, index)) = self.moved_tab(ws) {
            self.tab_placed(ws, pane, tab, index);
        }
    }

    fn moved_tab(&self, ws: WorkspaceId) -> Option<(TabId, PaneId, usize)> {
        let slots = self.slots.borrow();
        let model = self.model.borrow();
        let content = &model.spaces.get(&ws)?.content;
        let mut reordered = None;
        for (pane, view) in &slots.get(&ws)?.panes {
            for (index, body) in view.order().iter().enumerate() {
                let Some(tab) = self.tab_of(ws, body) else {
                    continue;
                };
                let (home, at) = content.find_tab(tab)?;
                if home != *pane {
                    return Some((tab, *pane, index));
                }
                if at != index && reordered.is_none() {
                    reordered = Some((tab, *pane, index));
                }
            }
        }
        reordered
    }

    pub(crate) fn tab_selected(self: &Rc<Self>, ws: WorkspaceId, pane: PaneId, tab: TabId) {
        let here = self
            .model
            .borrow()
            .spaces
            .get(&ws)
            .and_then(|s| s.content.find_tab(tab))
            .map(|(p, _)| p);
        if here == Some(pane) {
            self.select_tab_ui(ws, tab);
        }
    }

    pub(crate) fn tab_placed(
        self: &Rc<Self>,
        ws: WorkspaceId,
        pane: PaneId,
        tab: TabId,
        index: usize,
    ) {
        self.model
            .borrow_mut()
            .place_tab(ws, tab, pane, Some(index));
        self.changed(ws);
    }

    pub(crate) fn tab_dropped(
        self: &Rc<Self>,
        ws: WorkspaceId,
        pane: PaneId,
        tab: TabId,
        edge: Option<DropEdge>,
    ) {
        match edge {
            Some(edge) => self.model.borrow_mut().drop_on_edge(ws, tab, pane, edge),
            None => self.model.borrow_mut().place_tab(ws, tab, pane, None),
        };
        self.off_bar.set(true);
    }
}
