//! What the tab bar reports. Each event names a page; the app finds its tab,
//! asks the model, and syncs, the same path the keyboard commands take. Events
//! run on the next idle turn so the bar finishes its own drag first.

use std::rc::Rc;

use gtk::glib;
use xenon_core::{DropEdge, PaneId, TabId, WorkspaceId};

use crate::app::App;

impl App {
    /// Run `then` for the tab whose body is `child`, once the bar is done.
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

    /// The tab whose body is `child`.
    pub(crate) fn tab_of(&self, ws: WorkspaceId, child: &gtk::Widget) -> Option<TabId> {
        self.bodies
            .borrow()
            .iter()
            .find(|((w, _), body)| *w == ws && *body == child)
            .map(|((_, t), _)| *t)
    }

    /// A tab drag ended. A drop on a pane body is applied; a drop on a bar
    /// has already moved the page, so the model learns where it went.
    pub(crate) fn tab_transfer_done(self: &Rc<Self>, ws: WorkspaceId) {
        let refused = self.refused_drag.take();
        if let Some((pane, tab, edge)) = self.pending_drop.take() {
            self.tab_dropped(ws, pane, tab, edge);
            return;
        }
        if refused {
            self.sync(ws);
            return;
        }
        let moved = self.moved_tab(ws);
        if let Some((tab, pane, index)) = moved {
            self.tab_placed(ws, pane, tab, index);
        }
    }

    /// A tab that a view shows somewhere else than the model puts it: first
    /// one that changed pane, then one that changed place.
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

    /// The user picked a tab on the bar of `pane`.
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

    /// The user dragged `tab` to `index` of `pane`'s bar (reorder or move).
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

    /// The user dropped `tab` on `pane`'s body: an edge splits, the middle moves.
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
        self.changed(ws);
    }
}
