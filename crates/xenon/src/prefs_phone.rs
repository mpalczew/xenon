//! Settings > Phone Remote: on or off, keep-awake, address override, port,
//! pairing, and the paired devices (revoke one or all). The wording and the
//! status come from `xenon_remote`; the rows redraw when the status changes.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use xenon_remote::{DeviceBook, DeviceRow, RemoteStatus, ago};

use crate::app::App;

/// The line under the switch while the remote is off.
fn off_hint(app: &App) -> String {
    match app.chord("app.connect-phone") {
        Some(chord) => format!("Off \u{2014} {chord} connects a phone"),
        None => "Off".to_string(),
    }
}

/// Rows that change with the status; the device group is rebuilt.
struct Live {
    on: adw::SwitchRow,
    keep: adw::SwitchRow,
    hostname: adw::EntryRow,
    devices: adw::PreferencesGroup,
    device_rows: RefCell<Vec<gtk::Widget>>,
    /// True while we set a switch ourselves.
    setting: Cell<bool>,
}

pub(crate) fn page(app: &Rc<App>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .name("phone")
        .title("Phone Remote")
        .icon_name("phone-symbolic")
        .build();
    let group = adw::PreferencesGroup::new();
    let status = app.remote_status();
    let on = crate::prefs_pages::switch_row(
        "Phone remote",
        &status.subtitle(&off_hint(app)),
        status.enabled || status.waiting,
        |_| {},
    );
    let keep = crate::prefs_pages::switch_row(
        xenon_remote::KEEP_AWAKE_LABEL,
        xenon_remote::KEEP_AWAKE_DETAIL,
        status.keep_awake,
        |_| {},
    );
    let network = adw::ActionRow::builder()
        .title("Network")
        .subtitle("Tailscale or Headscale carries the encryption; plain HTTP on Wi-Fi would not")
        .build();
    network.add_suffix(&dim("Your tailnet only"));
    let hostname = adw::EntryRow::builder()
        .title("Address override")
        .tooltip_text("Host in the pairing QR code")
        .text(&status.hostname)
        .show_apply_button(true)
        .build();
    let port = adw::ActionRow::builder()
        .title("Port")
        .subtitle("Stays the same across restarts, so paired phones keep working")
        .build();
    port.add_suffix(&dim(&crate::remote::effective_port().to_string()));
    let pair = adw::ActionRow::builder()
        .title("Pair a phone")
        .subtitle("Shows a QR code to scan")
        .build();
    let connect = gtk::Button::with_label("Connect Phone\u{2026}");
    connect.add_css_class("suggested-action");
    connect.set_valign(gtk::Align::Center);
    pair.add_suffix(&connect);
    for row in [
        on.upcast_ref::<gtk::Widget>(),
        network.upcast_ref(),
        keep.upcast_ref(),
        hostname.upcast_ref(),
        port.upcast_ref(),
        pair.upcast_ref(),
    ] {
        group.add(row);
    }
    let devices = adw::PreferencesGroup::builder()
        .title("Paired devices")
        .build();
    page.add(&group);
    page.add(&devices);
    let live = Rc::new(Live {
        on: on.clone(),
        keep: keep.clone(),
        hostname: hostname.clone(),
        devices,
        device_rows: RefCell::default(),
        setting: Cell::new(false),
    });
    wire(app, &live, &connect);
    page
}

/// Hook the rows up to the app: toggles, hostname, Connect, and redraw on change.
fn wire(app: &Rc<App>, live: &Rc<Live>, connect: &gtk::Button) {
    let (on, keep, hostname) = (live.on.clone(), live.keep.clone(), live.hostname.clone());
    let weak = Rc::downgrade(app);
    {
        let (weak, live) = (weak.clone(), live.clone());
        on.connect_active_notify(move |row| {
            let Some(app) = weak.upgrade() else { return };
            let want = row.is_active();
            let now = app.remote.running() || app.remote.waiting();
            if !live.setting.get() && want != now {
                app.remote_toggle();
            }
        });
    }
    {
        let (weak, live) = (weak.clone(), live.clone());
        keep.connect_active_notify(move |row| {
            let Some(app) = weak.upgrade() else { return };
            let want = row.is_active();
            if live.setting.get() || want == app.remote_status().keep_awake {
                return;
            }
            if let Err(error) =
                xenon_store::update_settings(|s| s.remote_keep_awake_while_connected = want)
            {
                eprintln!("[xg] save keep-awake: {error}");
            }
            app.remote_publish();
        });
    }
    {
        let weak = weak.clone();
        hostname.connect_apply(move |row| {
            let host = xenon_remote::normalize_hostname(&row.text());
            if let Err(error) = xenon_store::update_settings(|s| s.remote_hostname = host.clone()) {
                eprintln!("[xg] save hostname: {error}");
            }
            if let Some(app) = weak.upgrade() {
                app.remote_publish();
            }
        });
    }
    {
        let weak = weak.clone();
        connect.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.connect_phone();
            }
        });
    }
    paint(live, app, &app.remote_status());
    let refresh = {
        let (weak, live) = (weak.clone(), live.clone());
        move || {
            if let Some(app) = weak.upgrade() {
                paint(&live, &app, &app.remote_status());
            }
        }
    };
    *app.remote.on_change.borrow_mut() = Some(Rc::new(refresh));
}

fn dim(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    label.set_valign(gtk::Align::Center);
    label
}

/// Bring the switches and the device list in line with `status`.
fn paint(live: &Rc<Live>, app: &Rc<App>, status: &RemoteStatus) {
    live.setting.set(true);
    live.on.set_active(status.enabled || status.waiting);
    live.on.set_subtitle(&status.subtitle(&off_hint(app)));
    live.keep.set_active(status.keep_awake);
    live.setting.set(false);
    let _ = &live.hostname;
    for old in live.device_rows.borrow_mut().drain(..) {
        live.devices.remove(&old);
    }
    let mut rows: Vec<gtk::Widget> = Vec::new();
    if status.devices.is_empty() {
        rows.push(
            adw::ActionRow::builder()
                .title("No phones paired yet")
                .subtitle("Connect Phone shows a QR code to scan")
                .build()
                .upcast(),
        );
    } else {
        rows.extend(status.devices.iter().map(|d| device_row(app, d)));
        let all = adw::ActionRow::builder()
            .title("Sign out every device")
            .subtitle("Old QR codes stop working too")
            .build();
        let button = gtk::Button::with_label("Sign out all");
        button.add_css_class("destructive-action");
        button.set_valign(gtk::Align::Center);
        let weak = Rc::downgrade(app);
        button.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.remote_revoke(None);
            }
        });
        all.add_suffix(&button);
        rows.push(all.upcast());
    }
    for row in &rows {
        live.devices.add(row);
    }
    *live.device_rows.borrow_mut() = rows;
}

fn device_row(app: &Rc<App>, device: &DeviceRow) -> gtk::Widget {
    let state = if device.connected {
        "\u{25cf} Connected now".to_string()
    } else {
        format!("Last seen {}", ago(device.last_seen_at))
    };
    let row = adw::ActionRow::builder()
        .title(&device.label)
        .subtitle(format!("{state} \u{b7} paired {}", ago(device.created_at)))
        .build();
    if device.connected {
        row.add_css_class("success");
    }
    let button = gtk::Button::with_label("Revoke");
    button.set_valign(gtk::Align::Center);
    let (weak, id) = (Rc::downgrade(app), device.id.clone());
    button.connect_clicked(move |_| {
        if let Some(app) = weak.upgrade() {
            app.remote_revoke(Some(&id));
        }
    });
    row.add_suffix(&button);
    row.upcast()
}

impl App {
    /// Forget one device (or all) on disk and close their live sessions.
    pub(crate) fn remote_revoke(self: &Rc<Self>, id: Option<&str>) {
        let mut book = self
            .remote
            .rt
            .borrow_mut()
            .as_mut()
            .map(|rt| {
                std::mem::replace(
                    &mut rt.devices,
                    DeviceBook::from_devices(Default::default()),
                )
            })
            .unwrap_or_else(DeviceBook::load);
        book.reload();
        match id {
            Some(id) => {
                book.revoke(id);
            }
            None => book.revoke_all(),
        }
        if let Err(error) = book.save() {
            self.toast(&format!("Couldn’t update paired devices: {error}"));
        }
        if let Some(rt) = self.remote.rt.borrow_mut().as_mut() {
            rt.devices = book;
        }
        self.remote_drop_devices(id);
        self.remote_publish();
    }
}
