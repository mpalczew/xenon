//! Dropping a dragged tab on a pane's body: the left, right, top and bottom
//! bands split the pane that way, the middle moves the tab there. The bands
//! are 40 px wide, lit while the tab hovers. `AdwTabBar` offers
//! the dragged `AdwTabPage` to a plain `gtk::DropTarget` (checked), so nothing
//! else is needed; ours runs in the capture phase because the bar puts a target
//! of its own on the view.

use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use xenon_core::{DropEdge, PaneId, WorkspaceId};

use crate::app::App;

/// Width of an edge band.
const EDGE: i32 = 40;

/// The band under (x, y); `None` is the middle. Left and right run the full
/// height, top and bottom sit between them.
fn zone(x: f64, y: f64, width: i32, height: i32) -> Option<DropEdge> {
    let (x, y) = (x as i32, y as i32);
    if x < EDGE {
        Some(DropEdge::Left)
    } else if x >= width - EDGE {
        Some(DropEdge::Right)
    } else if y < EDGE {
        Some(DropEdge::Top)
    } else if y >= height - EDGE {
        Some(DropEdge::Bottom)
    } else {
        None
    }
}

/// Place the lit rectangle over `edge` (or the middle).
fn light(marker: &gtk::Box, edge: Option<DropEdge>) {
    let fill = gtk::Align::Fill;
    let (h, v, w, t, inset_x, inset_y) = match edge {
        Some(DropEdge::Left) => (gtk::Align::Start, fill, EDGE, -1, 0, 0),
        Some(DropEdge::Right) => (gtk::Align::End, fill, EDGE, -1, 0, 0),
        Some(DropEdge::Top) => (fill, gtk::Align::Start, -1, EDGE, EDGE, 0),
        Some(DropEdge::Bottom) => (fill, gtk::Align::End, -1, EDGE, EDGE, 0),
        None => (fill, fill, -1, -1, EDGE, EDGE),
    };
    marker.set_halign(h);
    marker.set_valign(v);
    marker.set_width_request(w);
    marker.set_height_request(t);
    marker.set_margin_start(inset_x);
    marker.set_margin_end(inset_x);
    marker.set_margin_top(inset_y);
    marker.set_margin_bottom(inset_y);
    marker.set_visible(true);
}

pub(crate) fn install(app: &Rc<App>, body: &gtk::Overlay, ws: WorkspaceId, pane: PaneId) {
    let marker = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    marker.add_css_class("xdrop");
    marker.set_can_target(false);
    marker.set_visible(false);
    body.add_overlay(&marker);
    let target = gtk::DropTarget::new(adw::TabPage::static_type(), gdk::DragAction::MOVE);
    // The tab view has its own target that appends the page; ours goes first.
    target.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let marker = marker.clone();
        target.connect_motion(move |target, x, y| {
            if let Some(body) = target.widget() {
                // sync: pure geometry.
                let edge = zone(x, y, body.width(), body.height());
                let marker = marker.clone();
                gtk::glib::idle_add_local_once(move || light(&marker, edge));
            }
            gdk::DragAction::MOVE
        });
    }
    {
        let marker = marker.clone();
        target.connect_leave(move |_| {
            let marker = marker.clone();
            gtk::glib::idle_add_local_once(move || marker.set_visible(false));
        });
    }
    let weak = Rc::downgrade(app);
    // sync: must say at once whether the drop was taken; the model change must come before the bar's transfer-done idle.
    target.connect_drop(move |target, value, x, y| {
        let marker = marker.clone();
        gtk::glib::idle_add_local_once(move || marker.set_visible(false));
        let (Some(app), Some(body), Ok(page)) =
            (weak.upgrade(), target.widget(), value.get::<adw::TabPage>())
        else {
            return false;
        };
        let edge = zone(x, y, body.width(), body.height());
        let Some(tab) = app.tab_of(ws, &page.child()) else {
            return false;
        };
        // Only the model changes now; the views are redrawn from it once the
        // bar has put the page back (`tab_transfer_done`).
        app.tab_dropped(ws, pane, tab, edge);
        true
    });
    body.add_controller(target);
}
