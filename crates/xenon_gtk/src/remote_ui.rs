//! What the remote shows in the window: the sidebar footer line, the "Connect
//! Phone" dialog (QR, six digits, address), and the actions that open them.
//! The `remote-state` action publishes the same facts as JSON for scripts.

use std::rc::Rc;
use std::time::Instant;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use qrcode::{Color, QrCode};
use xenon_remote::{RemoteFooter, RemoteStatus, device_short_name, pairing_url};

use crate::app::App;

/// The sidebar footer: one flat button, shown only while the remote is on.
pub(crate) struct Footer {
    pub root: gtk::Button,
    label: gtk::Label,
}

impl Footer {
    pub(crate) fn new() -> Self {
        let label = gtk::Label::new(None);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&gtk::Image::from_icon_name("phone-symbolic"));
        row.append(&label);
        let root = gtk::Button::new();
        root.set_child(Some(&row));
        root.add_css_class("flat");
        root.set_action_name(Some("app.connect-phone"));
        root.set_tooltip_text(Some("Connect a phone"));
        root.set_focusable(false);
        root.set_visible(false);
        Self { root, label }
    }
}

impl App {
    /// What the footer says now (None while the remote is off).
    pub(crate) fn remote_footer(&self, status: &RemoteStatus) -> Option<RemoteFooter> {
        if status.waiting {
            return Some(RemoteFooter::Waiting);
        }
        let rt = self.remote.rt.borrow();
        let rt = rt.as_ref()?;
        let devices: Vec<String> = rt
            .conns
            .values()
            .map(|c| {
                device_short_name(rt.devices.label(&c.device_id).unwrap_or("Phone")).to_string()
            })
            .collect();
        let model = self.model.borrow();
        let driving = rt.conns.values().find_map(|c| {
            let a = c.attach.as_ref()?;
            let device = model.tab(a.ws, a.tab)?.phone_device.clone()?;
            Some((device, model.rec(a.ws)?.name.clone()))
        });
        Some(RemoteFooter::from_facts(false, &devices, driving))
    }

    pub(crate) fn remote_paint_footer(&self, status: &RemoteStatus) {
        let footer = self.remote_footer(status);
        let shown = &self.remote.footer;
        shown.root.set_visible(footer.is_some());
        if let Some(footer) = footer {
            shown.label.set_text(&footer.label());
            crate::pane::set_class(&shown.label, "accent", footer.active());
            crate::pane::set_class(&shown.label, "dim-label", !footer.active());
        }
    }

    /// The `remote-state` action: status, and the pairing code while the
    /// dialog is open.
    pub(crate) fn publish_remote_state(&self, status: &RemoteStatus) {
        let attached: Vec<serde_json::Value> = self
            .remote
            .rt
            .borrow()
            .as_ref()
            .map(|rt| {
                rt.conns
                    .values()
                    .filter_map(|c| c.attach.as_ref())
                    .map(|a| serde_json::json!({"tab": a.tab.0, "fit": a.fit}))
                    .collect()
            })
            .unwrap_or_default();
        let pairing = self.pairing_snapshot();
        let json = serde_json::json!({
            "enabled": status.enabled,
            "waiting": status.waiting,
            "connected": status.connected,
            "base_url": status.base_url,
            "devices": status.devices.iter().map(|d| {
                serde_json::json!({"label": d.label, "connected": d.connected})
            }).collect::<Vec<_>>(),
            "attached": attached,
            "pairing": pairing,
        })
        .to_string();
        let Some(action) = self
            .gtk
            .lookup_action("remote-state")
            .and_downcast::<gio::SimpleAction>()
        else {
            return;
        };
        if action.state().and_then(|s| s.get::<String>()).as_deref() != Some(json.as_str()) {
            action.set_state(&json.to_variant());
        }
        self.refresh_connect_dialog();
    }

    /// Digits and link of the live pairing code (None while no dialog shows it).
    fn pairing_snapshot(&self) -> Option<serde_json::Value> {
        self.remote.connect_dialog.borrow().as_ref()?;
        let (digits, url) = self.pairing_facts()?;
        Some(serde_json::json!({"digits": digits, "url": url}))
    }

    fn pairing_facts(&self) -> Option<(String, Option<String>)> {
        let mut guard = self.remote.rt.borrow_mut();
        let rt = guard.as_mut()?;
        let code = rt.devices.pairing_code(Instant::now()).clone();
        let settings = xenon_store::load_settings().unwrap_or_default();
        let override_host = xenon_remote::normalize_hostname(&settings.remote_hostname);
        let base = xenon_remote::base_url(&rt.reach.phone_host(&override_host), rt.port);
        let reachable = rt.reach.reachable() || !override_host.is_empty();
        Some((
            code.display_digits(),
            reachable.then(|| pairing_url(&base, &code.secret)),
        ))
    }

    /// "Connect Phone": turn the remote on if needed and show the dialog.
    pub(crate) fn connect_phone(self: &Rc<Self>) {
        if !self.remote.running() && !self.remote.waiting() {
            self.remote_start();
            crate::remote::persist_enabled(self.remote.running() || self.remote.waiting());
        }
        if self.remote.waiting() {
            self.toast("The other Xenon has the phone remote: connect from there, or quit it");
            return;
        }
        let open = self.remote.connect_dialog.borrow().clone();
        if let Some(dialog) = open {
            // The same key again closes it.
            dialog.close();
            return;
        }
        if !self.remote.running() {
            return;
        }
        let dialog = adw::Dialog::new();
        dialog.set_title("Connect Phone");
        dialog.set_content_width(420);
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            let Some(app) = weak.upgrade() else { return };
            *app.remote.connect_dialog.borrow_mut() = None;
            if let Some(rt) = app.remote.rt.borrow_mut().as_mut() {
                rt.devices.forget_pairing();
            }
            app.remote_publish();
        });
        *self.remote.connect_dialog.borrow_mut() = Some(dialog.clone());
        self.refresh_connect_dialog();
        dialog.present(Some(&self.win));
        self.remote_publish();
    }

    /// Redraw the dialog from the live code (a spent code is replaced).
    fn refresh_connect_dialog(&self) {
        let Some(dialog) = self.remote.connect_dialog.borrow().clone() else {
            return;
        };
        let Some((digits, url)) = self.pairing_facts() else {
            return;
        };
        let status = self.remote_status();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(content.upcast_ref(), 20);
        }
        match &url {
            Some(url) => {
                let qr = gtk::Picture::for_paintable(&qr_texture(url));
                qr.set_size_request(220, 220);
                qr.set_halign(gtk::Align::Center);
                qr.set_can_shrink(false);
                qr.set_tooltip_text(Some("Scan with your phone camera"));
                content.append(&qr);
                let hint = gtk::Label::new(Some("Scan with your phone camera"));
                hint.add_css_class("dim-label");
                content.append(&hint);
            }
            None => {
                let warn = gtk::Label::new(Some(
                    "Tailscale not detected, so your phone can't reach this computer. \
                     Set an address in Settings > Phone Remote, or connect to your tailnet.",
                ));
                warn.set_wrap(true);
                warn.add_css_class("warning");
                content.append(&warn);
            }
        }
        let code = gtk::Label::new(Some(&digits));
        code.add_css_class("title-1");
        code.add_css_class("monospace");
        code.set_selectable(true);
        content.append(&code);
        let sub = gtk::Label::new(Some(
            "Or type this code in the Home Screen app. It works once, for ten minutes.",
        ));
        sub.add_css_class("dim-label");
        sub.set_wrap(true);
        content.append(&sub);
        let address = gtk::Label::new(status.base_url.as_deref());
        address.set_selectable(true);
        address.add_css_class("monospace");
        content.append(&address);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::Center);
        let copy = gtk::Button::with_label("Copy link");
        copy.set_sensitive(url.is_some());
        let link = url.clone().unwrap_or_default();
        let weak = dialog.downgrade();
        copy.connect_clicked(move |button| {
            button.clipboard().set_text(&link);
            if let Some(dialog) = weak.upgrade() {
                dialog.close();
            }
        });
        let done = gtk::Button::with_label("Done");
        done.add_css_class("suggested-action");
        let weak = dialog.downgrade();
        done.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.close();
            }
        });
        buttons.append(&copy);
        buttons.append(&done);
        content.append(&buttons);
        dialog.set_child(Some(&content));
    }
}

/// The link as a QR code: black modules on white with a quiet border.
fn qr_texture(text: &str) -> gdk::MemoryTexture {
    const SCALE: usize = 6;
    const QUIET: usize = 2;
    let code = QrCode::new(text.as_bytes()).ok();
    let (width, colors) = code
        .as_ref()
        .map_or((0, Vec::new()), |c| (c.width(), c.to_colors()));
    let side = (width + 2 * QUIET) * SCALE;
    let mut pixels = vec![255u8; side * side * 4];
    for y in 0..width {
        for x in 0..width {
            if colors[y * width + x] != Color::Dark {
                continue;
            }
            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let px = (x + QUIET) * SCALE + dx;
                    let py = (y + QUIET) * SCALE + dy;
                    let at = (py * side + px) * 4;
                    pixels[at..at + 3].copy_from_slice(&[0, 0, 0]);
                }
            }
        }
    }
    gdk::MemoryTexture::new(
        side as i32,
        side as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(pixels),
        side * 4,
    )
}

pub(crate) fn install(app: &Rc<App>) {
    app.gtk.add_action(&gio::SimpleAction::new_stateful(
        "remote-state",
        None,
        &"".to_variant(),
    ));
    let connect = gio::SimpleAction::new("connect-phone", None);
    let weak = Rc::downgrade(app);
    connect.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.connect_phone();
        }
    });
    app.gtk.add_action(&connect);
    let toggle = gio::SimpleAction::new("remote-toggle", None);
    let weak = Rc::downgrade(app);
    toggle.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.remote_toggle();
        }
    });
    app.gtk.add_action(&toggle);
}
