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

    pub(crate) fn on(&self) -> bool {
        self.status.borrow().on()
    }
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

    pub(crate) fn remote_toggle(self: &Rc<Self>) {
        let want = !self.remote.on();
        if let Err(error) = xenon_store::update_service_settings(|s| s.remote_enabled = want) {
            self.toast(&format!("Phone remote didn\u{2019}t change: {error}"));
        }
        self.remote_publish();
        let status = self.remote_status();
        let text = match (status.enabled, &status.problem) {
            (true, _) => "Phone remote on".to_string(),
            (false, Some(problem)) => format!("Phone remote didn\u{2019}t start: {problem}"),
            (false, None) => "Phone remote off".to_string(),
        };
        self.toast(&text);
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
