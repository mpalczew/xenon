//! The phone remote as the window sees it: a view over the phone server that runs
//! as its own process (`xenon_phone::PhoneLink`). The window is the only writer of
//! `services.json`, where the phone server's settings live.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::GtkApplicationExt;
use xenon_remote::RemoteStatus;

use crate::app::App;

pub(crate) struct Remote {
    status: RefCell<RemoteStatus>,
    pub on_change: RefCell<Option<Rc<dyn Fn()>>>,
    pub connect_dialog: RefCell<Option<adw::Dialog>>,
    keep_awake: Cell<Option<u32>>,
}

impl Remote {
    pub(crate) fn new() -> Self {
        Self {
            status: RefCell::default(),
            on_change: RefCell::new(None),
            connect_dialog: RefCell::new(None),
            keep_awake: Cell::new(None),
        }
    }

    pub(crate) fn running(&self) -> bool {
        self.status.borrow().enabled
    }

    pub(crate) fn waiting(&self) -> bool {
        self.status.borrow().waiting
    }
}

pub(crate) fn effective_port() -> u16 {
    let saved = xenon_store::load_service_settings()
        .unwrap_or_default()
        .remote_port;
    xenon_remote::effective_port(saved, xenon_store::DEFAULT_REMOTE_PORT)
}

impl App {
    pub(crate) fn remote_boot(self: &Rc<Self>) {
        self.remote_publish();
        let changes = xenon_phone::PhoneLink::watch();
        let weak = Rc::downgrade(self);
        gtk::glib::spawn_future_local(async move {
            while changes.recv().await.is_ok() {
                let Some(app) = weak.upgrade() else { break };
                app.remote_publish();
            }
        });
    }

    /// Turn the phone server on or off: the choice is written to `services.json`, which
    /// the phone server reads.
    pub(crate) fn remote_set(self: &Rc<Self>, on: bool) {
        if let Err(error) = xenon_store::update_service_settings(|s| s.remote_enabled = on) {
            self.toast(&format!("Phone remote didn\u{2019}t change: {error}"));
        }
        self.remote_publish();
    }

    pub(crate) fn remote_toggle(self: &Rc<Self>) {
        let want = !(self.remote.running() || self.remote.waiting());
        self.remote_set(want);
        let text = match (self.remote.running(), self.remote.waiting()) {
            (true, _) => "Phone remote on",
            (false, true) => {
                "Phone remote on \u{2014} its port is busy until the other program quits"
            }
            (false, false) => "Phone remote off",
        };
        self.toast(text);
    }

    pub(crate) fn remote_status(&self) -> RemoteStatus {
        self.remote.status.borrow().clone()
    }

    pub(crate) fn remote_publish(self: &Rc<Self>) {
        match xenon_phone::PhoneLink::status() {
            Ok(status) => *self.remote.status.borrow_mut() = status,
            Err(error) => eprintln!("[xg] phone status: {error}"),
        }
        let hold = self.remote.status.borrow().hold_awake();
        self.remote_keep_awake(hold);
        self.refresh_connect_dialog();
        let callback = self.remote.on_change.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    pub(crate) fn remote_revoke(self: &Rc<Self>, id: Option<&str>) {
        match xenon_phone::PhoneLink::revoke(id) {
            Ok(status) => *self.remote.status.borrow_mut() = status,
            Err(error) => self.toast(&format!("Couldn\u{2019}t update paired devices: {error}")),
        }
        self.remote_publish();
    }

    fn remote_keep_awake(&self, hold: bool) {
        match (hold, self.remote.keep_awake.get()) {
            (true, None) => {
                let cookie = self.gtk.inhibit(
                    None::<&gtk::Window>,
                    gtk::ApplicationInhibitFlags::IDLE,
                    Some("A phone is connected to Xenon"),
                );
                self.remote.keep_awake.set((cookie != 0).then_some(cookie));
            }
            (false, Some(cookie)) => {
                self.gtk.uninhibit(cookie);
                self.remote.keep_awake.set(None);
            }
            _ => {}
        }
    }
}
