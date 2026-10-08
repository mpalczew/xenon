//! "Connect Phone" sheet (⌘⇧M): QR to the one-time pairing link, the same
//! code as six digits for the Home Screen app, and the address. Keyboard:
//! Escape/Return close, ⌘C copies the link.

use std::time::Instant;

use gpui::{AnyElement, Bounds, canvas, fill, point, size};
use qrcode::{Color as QrColor, QrCode};
use xenon_design_system::{ActionButton, FocusOnOpen, TypeRole, Typography};

use super::network::pairing_url;
use super::*;

const QR_SIZE: f32 = 200.;
const QR_QUIET_MODULES: usize = 2;

pub(crate) struct PairingSheet {
    focus: FocusOnOpen,
}

/// What the sheet paints (resolved before layout so render stays pure).
struct SheetModel {
    url: Option<String>,
    digits: String,
    expires_in: Duration,
    address: String,
    reachable: bool,
}

impl XenonApp {
    /// Turn the remote on if needed and show the sheet.
    pub(crate) fn open_connect_phone(&mut self, cx: &mut Context<Self>) {
        if !self.remote_running() && !self.remote_waiting() {
            self.start_mobile_remote(cx);
            persist_enabled(self.remote_running() || self.remote_waiting());
        }
        if self.remote_waiting() {
            let toast = xenon_design_system::Toast::info(
                "📱",
                xenon_design_system::shortcut_text(
                    "The other Xenon has the phone remote — ⌘⇧M there, or quit it",
                ),
            );
            self.show_toast(toast, cx);
            return;
        }
        let Some(runtime) = self.services.remote.as_mut() else {
            return;
        };
        runtime.devices.pairing_code(Instant::now());
        let mut focus = FocusOnOpen::new(cx.focus_handle());
        focus.open();
        self.pairing_sheet = Some(PairingSheet { focus });
        if self.deferred.restore_pane.is_none() {
            self.deferred.restore_pane = Some(FocusOwner::Terminal);
        }
        self.publish_remote_info(cx);
    }

    pub(crate) fn close_connect_phone(&mut self, cx: &mut Context<Self>) {
        if let Some(runtime) = self.services.remote.as_mut() {
            runtime.devices.forget_pairing();
        }
        if self.pairing_sheet.take().is_some() {
            self.deferred.pending_focus = self.deferred.restore_pane.take();
            cx.notify();
        }
    }

    pub(in crate::app) fn focus_pairing_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(sheet) = self.pairing_sheet.as_mut() {
            sheet.focus.focus_after_open(window, cx);
        }
    }

    fn sheet_model(&mut self) -> Option<SheetModel> {
        let runtime = self.services.remote.as_mut()?;
        let now = Instant::now();
        let code = runtime.devices.pairing_code(now).clone();
        let hostname = xenon_store::load_settings()
            .map(|s| normalize_hostname(&s.remote_hostname))
            .unwrap_or_default();
        let host = runtime.reach.phone_host(runtime.network, &hostname);
        let base = network::base_url(&host, runtime.port);
        let reachable = runtime.reach.reachable(runtime.network) || !hostname.is_empty();
        Some(SheetModel {
            url: reachable.then(|| pairing_url(&base, &code.secret)),
            digits: code.display_digits(),
            expires_in: if cfg!(feature = "visual-tests") {
                Duration::from_secs(581)
            } else {
                code.expires_at.saturating_duration_since(now)
            },
            address: base,
            reachable,
        })
    }

    fn copy_pairing_link(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.sheet_model().and_then(|m| m.url) else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(url));
        self.show_toast(toasts::copied("pairing link"), cx);
    }

    fn on_pairing_sheet_key(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let ks = &event.keystroke;
        match ks.key.as_str() {
            "escape" | "enter" => self.close_connect_phone(cx),
            "c" if ks.modifiers.secondary() => self.copy_pairing_link(cx),
            _ => return false,
        }
        true
    }

    pub(crate) fn render_pairing_sheet(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let focus = self.pairing_sheet.as_ref()?.focus.handle();
        let model = self.sheet_model()?;
        let colors = cx.theme().colors().clone();
        let dialog = div()
            .id("connect-phone-dialog")
            .occlude()
            .track_focus(&focus)
            .key_context("ConnectPhone")
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if this.on_pairing_sheet_key(event, cx) {
                    cx.stop_propagation();
                }
            }))
            .w(px(580.))
            .p_5()
            .rounded_lg()
            .bg(colors.elevated_surface_background)
            .border_1()
            .border_color(colors.border)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .type_role(TypeRole::SectionTitle, cx)
                    .child("Connect Phone"),
            )
            .child(
                div()
                    .flex()
                    .gap_5()
                    .child(qr_block(model.url.as_deref(), model.reachable))
                    .child(sheet_steps(&model, cx)),
            )
            .child(div().h(px(1.)).bg(colors.border))
            .child(self.sheet_actions(model.url.is_some(), cx));
        Some(
            div()
                .id("connect-phone-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.background.opacity(0.55))
                .on_click(cx.listener(|this, _, _, cx| this.close_connect_phone(cx)))
                .child(dialog)
                .into_any_element(),
        )
    }

    fn sheet_actions(&self, can_copy: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let copy = xenon_design_system::action_button(
            "connect-phone-copy",
            ActionButton::secondary(xenon_design_system::shortcut_text("Copy link  ⌘C")),
            cx,
            cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.copy_pairing_link(cx);
            }),
        );
        div()
            .flex()
            .justify_end()
            .gap_2()
            .children(can_copy.then_some(copy))
            .child(xenon_design_system::action_button(
                "connect-phone-settings",
                ActionButton::secondary(xenon_design_system::shortcut_text("Settings…  ⌘,")),
                cx,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.close_connect_phone(cx);
                    this.toggle_settings_window(cx);
                }),
            ))
            .child(xenon_design_system::action_button(
                "connect-phone-done",
                ActionButton::primary("Done  esc"),
                cx,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.close_connect_phone(cx);
                }),
            ))
    }
}

fn sheet_steps(model: &SheetModel, cx: &App) -> impl IntoElement + use<> {
    let colors = cx.theme().colors().clone();
    let muted = colors.text_muted;
    let line = |text: &'static str| div().type_role(TypeRole::Body, cx).child(text);
    if !model.reachable {
        return div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(crate::chrome::status_color(cx, true).opacity(0.14))
                    .text_color(crate::chrome::status_color(cx, true))
                    .type_role(TypeRole::ControlLabel, cx)
                    .child("Tailscale isn’t running on this Mac, so your phone can’t reach Xenon away from home."),
            )
            .child(
                div()
                    .type_role(TypeRole::Supporting, cx)
                    .text_color(muted)
                    .child("Open Tailscale and sign in; this sheet updates on its own. Or allow Tailscale + LAN in Settings to use Xenon on home Wi-Fi."),
            );
    }
    let secs = model.expires_in.as_secs();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .flex_1()
        .min_w_0()
        .child(line("1. Scan with your phone’s camera."))
        .child(line("2. In Safari: Share › Add to Home Screen."))
        .child(line("3. Opening the Home Screen app? Enter:"))
        .child(
            div()
                .font_family("Menlo")
                .text_size(px(26.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(model.digits.clone()),
        )
        .child(
            div()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(muted)
                .child(format!(
                    "One-time · expires in {}:{:02}",
                    secs / 60,
                    secs % 60
                )),
        )
        .child(
            div()
                .mt_2()
                .type_role(TypeRole::ControlLabel, cx)
                .text_color(muted)
                .child("Address · phone must be on Tailscale"),
        )
        .child(
            div()
                .type_role(TypeRole::Code, cx)
                .text_color(colors.text_accent)
                .truncate()
                .child(model.address.clone()),
        )
}

/// White card with the QR; greyed placeholder when the phone can't reach us.
fn qr_block(url: Option<&str>, reachable: bool) -> impl IntoElement + use<> {
    let modules = url
        .and_then(|u| QrCode::new(u.as_bytes()).ok())
        .map(|code| {
            let width = code.width();
            let dark: Vec<bool> = code
                .to_colors()
                .into_iter()
                .map(|c| c == QrColor::Dark)
                .collect();
            (width, dark)
        });
    div()
        .flex_none()
        .size(px(QR_SIZE))
        .rounded_md()
        .bg(gpui::white())
        .when(!reachable, |d| d.opacity(0.25))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    if let Some((width, dark)) = &modules {
                        paint_qr(bounds, *width, dark, window);
                    }
                },
            )
            .size_full(),
        )
}

fn paint_qr(bounds: Bounds<Pixels>, width: usize, dark: &[bool], window: &mut Window) {
    let span = (width + 2 * QR_QUIET_MODULES) as f32;
    let module = bounds.size.width / span;
    for (i, _) in dark.iter().enumerate().filter(|(_, d)| **d) {
        let (x, y) = (i % width + QR_QUIET_MODULES, i / width + QR_QUIET_MODULES);
        let origin = point(
            bounds.origin.x + module * x as f32,
            bounds.origin.y + module * y as f32,
        );
        // Slight overlap hides hairline seams between adjacent modules.
        let side = module + px(0.5);
        window.paint_quad(fill(Bounds::new(origin, size(side, side)), gpui::black()));
    }
}
