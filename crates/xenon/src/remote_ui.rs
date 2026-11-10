use std::rc::Rc;
use std::time::Instant;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use qrcode::{Color, QrCode};
use xenon_remote::pairing_url;

use crate::app::{App, defer};

impl App {
    fn pairing_facts(&self) -> Option<(String, Option<String>, Instant)> {
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
            code.expires_at,
        ))
    }

    pub(crate) fn connect_phone(self: &Rc<Self>) {
        if !self.remote.running() && !self.remote.waiting() {
            self.remote_start();
            crate::remote::persist_enabled(self.remote.running() || self.remote.waiting());
        }
        let open = self.remote.connect_dialog.borrow().clone();
        if let Some(dialog) = open {
            dialog.close();
            return;
        }
        if !self.remote.running() && !self.remote.waiting() {
            return;
        }
        let dialog = adw::Dialog::new();
        dialog.set_title("Connect Phone");
        dialog.set_content_width(420);
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            let Some(app) = weak.upgrade() else { return };
            glib::idle_add_local_once(move || {
                *app.remote.connect_dialog.borrow_mut() = None;
                if let Some(rt) = app.remote.rt.borrow_mut().as_mut() {
                    rt.devices.forget_pairing();
                }
                app.remote_publish();
            });
        });
        *self.remote.connect_dialog.borrow_mut() = Some(dialog.clone());
        self.refresh_connect_dialog();
        dialog.present(Some(&self.win));
        self.remote_publish();
    }

    pub(crate) fn refresh_connect_dialog(&self) {
        let Some(dialog) = self.remote.connect_dialog.borrow().clone() else {
            return;
        };
        let facts = self.pairing_facts();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        for set in [
            gtk::Widget::set_margin_top,
            gtk::Widget::set_margin_bottom,
            gtk::Widget::set_margin_start,
            gtk::Widget::set_margin_end,
        ] {
            set(content.upcast_ref(), 20);
        }
        let buttons = self.connect_buttons(&dialog, facts.as_ref().and_then(|f| f.1.clone()));
        let Some((digits, url, expires_at)) = facts else {
            let warn = gtk::Label::new(Some(&match self.chord("app.connect-phone") {
                Some(chord) => format!(
                    "The other Xenon has the phone remote. Press {chord} there, or quit it to use the phone remote here."
                ),
                None => {
                    "The other Xenon has the phone remote. Quit it to use the phone remote here."
                        .to_string()
                }
            }));
            warn.set_wrap(true);
            warn.add_css_class("warning");
            content.append(&warn);
            content.append(&buttons);
            dialog.set_child(Some(&content));
            return;
        };
        let status = self.remote_status();
        match &url {
            Some(url) => {
                let qr = gtk::Picture::for_paintable(&qr_texture(url));
                qr.set_size_request(220, 220);
                qr.set_halign(gtk::Align::Center);
                qr.set_can_shrink(false);
                content.append(&qr);
                for line in [
                    "1. Scan with your phone\u{2019}s camera.",
                    "2. In Safari: Share \u{203a} Add to Home Screen.",
                    "3. Opening the Home Screen app? Enter:",
                ] {
                    let label = gtk::Label::new(Some(line));
                    label.set_xalign(0.0);
                    content.append(&label);
                }
            }
            None => {
                let warn = gtk::Label::new(Some(
                    "Tailscale isn\u{2019}t running on this computer, so your phone can\u{2019}t reach Xenon.",
                ));
                warn.set_wrap(true);
                warn.add_css_class("warning");
                content.append(&warn);
                let help = gtk::Label::new(Some(
                    "Open Tailscale (or your Headscale client) and sign in; this sheet updates on its own. The phone remote only listens on your tailnet.",
                ));
                help.set_wrap(true);
                help.add_css_class("dim-label");
                content.append(&help);
            }
        }
        let code = gtk::Label::new(Some(&digits));
        code.add_css_class("title-1");
        code.add_css_class("monospace");
        code.set_selectable(true);
        content.append(&code);
        let sub = expiry_label(expires_at);
        content.append(&sub);
        let heading = gtk::Label::new(Some("Address \u{b7} phone must be on Tailscale"));
        heading.add_css_class("dim-label");
        content.append(&heading);
        let address = gtk::Label::new(status.base_url.as_deref());
        address.set_selectable(true);
        address.add_css_class("monospace");
        content.append(&address);
        content.append(&buttons);
        dialog.set_child(Some(&content));
    }

    fn connect_buttons(&self, dialog: &adw::Dialog, url: Option<String>) -> gtk::Box {
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::Center);

        let settings = gtk::Button::with_label(&match self.chord("app.settings") {
            Some(chord) => format!("Settings\u{2026}  {chord}"),
            None => "Settings\u{2026}".to_string(),
        });
        let weak = dialog.downgrade();
        let gtk_app = self.gtk.clone();
        settings.connect_clicked(move |_| {
            let (dialog, gtk_app) = (weak.upgrade(), gtk_app.clone());
            glib::idle_add_local_once(move || {
                if let Some(dialog) = dialog {
                    dialog.close();
                }
                gtk_app.activate_action("settings", None);
            });
        });
        let done = gtk::Button::with_label("Done  esc");
        done.add_css_class("suggested-action");
        let weak = dialog.downgrade();
        done.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                glib::idle_add_local_once(move || {
                    dialog.close();
                });
            }
        });
        if let Some(url) = url {
            buttons.append(&copy_button(dialog, url));
        }
        buttons.append(&settings);
        buttons.append(&done);
        buttons
    }
}

fn copy_button(dialog: &adw::Dialog, link: String) -> gtk::Button {
    let copy = gtk::Button::with_label("Copy link");
    let weak = dialog.downgrade();
    copy.connect_clicked(move |button| {
        let (clipboard, link, dialog) = (button.clipboard(), link.clone(), weak.upgrade());
        glib::idle_add_local_once(move || {
            clipboard.set_text(&link);
            if let Some(dialog) = dialog {
                dialog.close();
            }
        });
    });
    copy
}

fn expiry_label(expires_at: Instant) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("dim-label");
    let tick = {
        let label = label.downgrade();
        move || {
            let Some(label) = label.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let secs = expires_at
                .saturating_duration_since(Instant::now())
                .as_secs();
            label.set_text(&format!(
                "One-time \u{b7} expires in {}:{:02}",
                secs / 60,
                secs % 60
            ));
            glib::ControlFlow::Continue
        }
    };
    tick();
    glib::timeout_add_seconds_local(1, tick);
    label
}

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
        defer(&weak, move |app| app.remote_toggle());
    });
    app.gtk.add_action(&toggle);
}
