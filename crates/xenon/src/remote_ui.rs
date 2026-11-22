use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::glib;
use qrcode::QrCode;

use crate::app::App;
use crate::commands::Cmd;

impl App {
    fn pairing_facts(&self) -> Option<(String, Option<String>, Instant)> {
        xenon_store::update_service_settings(|s| s.remote_enabled = true).ok()?;
        let pairing = xenon_phone::PhoneLink::pairing().ok()?;
        let expires_at = Instant::now() + Duration::from_secs(pairing.expires_in_secs);
        Some((pairing.digits, pairing.url, expires_at))
    }

    pub(crate) fn pair_text(self: &Rc<Self>) -> Result<String, String> {
        if let Err(error) = xenon_store::update_service_settings(|s| s.remote_enabled = true) {
            return Err(format!("could not turn the phone remote on: {error}"));
        }
        let pairing = xenon_phone::PhoneLink::pairing()?;
        self.remote_publish();
        let url = pairing
            .url
            .ok_or("your phone can't reach this computer: Tailscale isn't running")?;
        let qr = QrCode::new(url.as_bytes())
            .map_err(|e| format!("cannot make a QR code: {e}"))?
            .render::<qrcode::render::unicode::Dense1x2>()
            .dark_color(qrcode::render::unicode::Dense1x2::Light)
            .light_color(qrcode::render::unicode::Dense1x2::Dark)
            .build();
        Ok(format!(
            "Pairing code: {}\nExpires in 10 minutes.\n{url}\n\n{qr}",
            pairing.digits
        ))
    }

    pub(crate) fn connect_phone(self: &Rc<Self>) {
        self.remote_publish();
        let open = self.remote.connect_dialog.borrow().clone();
        if let Some(dialog) = open {
            dialog.close();
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
            let warn = gtk::Label::new(Some(
                "The phone remote isn\u{2019}t running: its port may be in use by another program, such as an older Xenon.",
            ));
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
                let qr = gtk::Picture::for_paintable(&qr_image(url));
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

        let settings = gtk::Button::with_label("Settings");
        Cmd::Settings.bind(&settings);
        let weak = dialog.downgrade();
        settings.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                glib::idle_add_local_once(move || {
                    dialog.close();
                });
            }
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

fn qr_image(text: &str) -> gtk::Svg {
    let svg = QrCode::new(text.as_bytes())
        .map(|code| {
            code.render::<qrcode::render::svg::Color>()
                .min_dimensions(220, 220)
                .build()
        })
        .unwrap_or_default();
    gtk::Svg::from_bytes(&glib::Bytes::from_owned(svg.into_bytes()))
}
