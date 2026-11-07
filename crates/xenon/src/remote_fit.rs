//! Phone fit: while a phone drives a terminal, the terminal takes the phone's
//! grid; the Mac's own keyboard takes it back, and it returns after 30 quiet
//! seconds. The rules are `xenon_remote::FitGate`'s; this applies them.

use std::rc::Rc;
use std::time::Instant;

use adw::prelude::*;
use gtk::glib;
use vte::prelude::*;
use xenon_core::{TabId, WorkspaceId};
use xenon_remote::{FitGate, device_short_name, smallest_fit};

use crate::app::App;
use crate::term::TermView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FitReq {
    pub cols: u16,
    pub rows: u16,
    /// Short device name for the tab.
    pub device: String,
}

#[derive(Default)]
pub(crate) struct TabFit {
    gate: FitGate<FitReq>,
    /// Bumped on every re-arm so an older timer does nothing.
    generation: u32,
}

impl TermView {
    /// Give the terminal exactly `cols` x `rows` (as far as the pane allows)
    /// instead of filling its pane.
    fn pin_size(&self, cols: u16, rows: u16) {
        self.term.set_hexpand(false);
        self.term.set_vexpand(false);
        self.term.set_halign(gtk::Align::Start);
        self.term.set_valign(gtk::Align::Start);
        self.term.set_size(i64::from(cols), i64::from(rows));
    }

    /// Fill the pane again; the grid follows the allocation.
    fn unpin_size(&self) {
        self.term.set_hexpand(true);
        self.term.set_vexpand(true);
        self.term.set_halign(gtk::Align::Fill);
        self.term.set_valign(gtk::Align::Fill);
    }
}

impl App {
    /// The smallest grid among the phones attached to this terminal, or none.
    pub(crate) fn remote_tab_fit(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let request = {
            let rt = self.remote.rt.borrow();
            rt.as_ref().and_then(|rt| {
                smallest_fit(rt.conns.values().filter_map(|c| {
                    let a = c.attach.as_ref()?;
                    let (cols, rows) = a.fit.filter(|_| a.ws == ws && a.tab == tab)?;
                    Some((cols, rows, c.device_id.clone()))
                }))
                .map(|(cols, rows, device)| FitReq {
                    cols,
                    rows,
                    device: device_short_name(rt.devices.label(&device).unwrap_or("Phone"))
                        .to_string(),
                })
            })
        };
        {
            let mut fits = self.remote.fits.borrow_mut();
            let entry = fits.entry((ws, tab)).or_default();
            entry.gate.set(request);
            if entry.gate.requested().is_none() {
                fits.remove(&(ws, tab));
            }
        }
        self.fit_apply(ws, tab);
    }

    /// Keyboard input in a terminal: hand the grid back if a phone had it.
    /// One map lookup when no phone is fitted.
    pub(crate) fn remote_local_input(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let took = match self.remote.fits.borrow_mut().get_mut(&(ws, tab)) {
            Some(entry) => entry.gate.note_local_input(Instant::now()),
            None => return,
        };
        if took {
            self.fit_apply(ws, tab);
            self.remote_publish();
        }
        self.fit_arm(ws, tab);
    }

    /// Make the terminal and its tab match what the gate says now.
    pub(crate) fn fit_apply(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let active = self
            .remote
            .fits
            .borrow()
            .get(&(ws, tab))
            .and_then(|e| e.gate.active(Instant::now()).cloned());
        if let Some(view) = self.terms.borrow().get(&(ws, tab)).cloned() {
            match &active {
                Some(fit) => view.pin_size(fit.cols, fit.rows),
                None => view.unpin_size(),
            }
        }
        let changed = self
            .model
            .borrow_mut()
            .set_phone_device(ws, tab, active.map(|f| f.device));
        if changed {
            self.relabel(ws, tab);
        }
        self.fit_arm(ws, tab);
        self.remote_poke_mirrors(ws, tab);
    }

    /// While a fit waits out the Mac's typing, apply it when that ends.
    fn fit_arm(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let (wait, generation) = {
            let mut fits = self.remote.fits.borrow_mut();
            let Some(entry) = fits.get_mut(&(ws, tab)) else {
                return;
            };
            entry.generation = entry.generation.wrapping_add(1);
            (entry.gate.wait_left(Instant::now()), entry.generation)
        };
        let Some(wait) = wait else { return };
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(wait, move || {
            let Some(app) = weak.upgrade() else { return };
            let current = app
                .remote
                .fits
                .borrow()
                .get(&(ws, tab))
                .is_some_and(|e| e.generation == generation);
            if current {
                app.fit_apply(ws, tab);
            }
        });
    }
}
