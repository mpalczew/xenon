//! Settings Remote section: mobile PTY remote toggle, host, password, URLs.

use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use theme::ActiveTheme;

use super::SettingsView;
use super::remote_edit::RemoteEditField;
use super::sections::{group_card, row_divider};

/// Active remote field edit for paint: (field, before|selected|after, empty).
type RemoteEditPaint = (RemoteEditField, (String, String, String), bool);

pub(super) fn remote_section(
    info: &crate::app::remote::MobileRemoteInfo,
    focused: bool,
    remote_edit: Option<RemoteEditPaint>,
    caret_on: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let subtitle = if info.enabled {
        "On — copy a URL below for your phone"
    } else {
        "Off — set host + password, then enable"
    };
    let enabled = info.enabled;
    let port = info.port;
    let token = info.token.clone();
    let hostname = info.hostname.clone();
    let urls = info.urls.clone();
    let editing = remote_edit.is_some();
    let password_paint = remote_edit
        .as_ref()
        .and_then(|(f, parts, empty)| (*f == RemoteEditField::Password).then_some((parts, *empty)));
    let hostname_paint = remote_edit
        .as_ref()
        .and_then(|(f, parts, empty)| (*f == RemoteEditField::Hostname).then_some((parts, *empty)));
    let body = div()
        .flex()
        .flex_col()
        .child(remote_toggle_row(
            enabled,
            subtitle,
            focused && !editing,
            cx,
        ))
        .child(row_divider(cx))
        .child(remote_hostname_block(
            &hostname,
            hostname_paint,
            caret_on,
            cx,
        ))
        .child(remote_password_block(&token, password_paint, caret_on, cx))
        .child(
            div()
                .px_3()
                .pb_2()
                .text_xs()
                .text_color(colors.text_muted)
                .child(format!("Port {port} (stable across restarts)")),
        )
        .when(enabled || !token.is_empty(), |col| {
            let mut details = div().flex().flex_col().gap_1().px_3().py_2().child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("URL — click to copy (includes password)"),
            );
            if urls.is_empty() {
                let fallback = if token.is_empty() {
                    format!("http://127.0.0.1:{port}/")
                } else {
                    crate::app::remote::remote_urls_for(port, &token, &hostname)
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| format!("http://127.0.0.1:{port}/"))
                };
                details = details.child(copyable_mono_row("remote-url-fallback", fallback, cx));
            } else {
                for (i, url) in urls.iter().enumerate() {
                    details = details.child(copyable_mono_row(
                        SharedString::from(format!("remote-url-{i}")),
                        url.clone(),
                        cx,
                    ));
                }
            }
            col.child(row_divider(cx)).child(details)
        });
    group_card("Remote", body, cx)
}

struct FieldEditPaint<'a> {
    id: &'static str,
    label: &'static str,
    parts: &'a (String, String, String),
    empty: bool,
    placeholder: &'static str,
    caret_on: bool,
}

fn remote_field_edit_row(
    paint: FieldEditPaint<'_>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let FieldEditPaint {
        id,
        label,
        parts,
        empty,
        placeholder,
        caret_on,
    } = paint;
    let colors = cx.theme().colors().clone();
    let (before, selected, after) = parts;
    // Caret between before and after (or after selected when range is non-empty).
    // Always reserve a 2px slot so blink doesn't reflow.
    let caret_color = if caret_on {
        colors.text
    } else {
        gpui::transparent_black()
    };
    let sel_bg = colors.element_selected;
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .child(
            div()
                .text_xs()
                .text_color(colors.text_muted)
                .child(format!("{label} — ←→ · ↵ save · Esc cancel")),
        )
        .child(
            div()
                .id(id)
                .flex()
                .items_center()
                .h(px(28.))
                .rounded_sm()
                .px_2()
                .border_1()
                .border_color(colors.border_focused)
                .bg(colors.elevated_surface_background)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_sm()
                        .font_family("Menlo")
                        .text_color(if empty {
                            colors.text_muted
                        } else {
                            colors.text
                        })
                        .when(empty, |d| d.child(placeholder))
                        .when(!empty, |d| {
                            d.child(before.clone())
                                .when(!selected.is_empty(), |d| {
                                    d.child(div().bg(sel_bg).child(selected.clone()))
                                })
                                .child(div().flex_none().w(px(2.)).h(px(14.)).bg(caret_color))
                                .child(after.clone())
                        }),
                ),
        )
}

struct FieldStaticPaint {
    id: &'static str,
    label: &'static str,
    empty_hint: &'static str,
}

fn remote_field_static_row(
    paint: FieldStaticPaint,
    current: &str,
    on_edit: impl Fn(&mut SettingsView, &mut gpui::Window, &mut Context<SettingsView>) + 'static,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let FieldStaticPaint {
        id,
        label,
        empty_hint,
    } = paint;
    let colors = cx.theme().colors().clone();
    let empty = current.is_empty();
    let display = if empty {
        empty_hint.to_string()
    } else {
        current.to_string()
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .child(
            div()
                .text_xs()
                .text_color(colors.text_muted)
                .child(format!("{label} — click to edit")),
        )
        .child(
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .h(px(28.))
                .rounded_sm()
                .px_2()
                .cursor_pointer()
                .hover(|s| s.bg(colors.element_hover))
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    on_edit(this, window, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_sm()
                        .font_family("Menlo")
                        .text_color(if empty {
                            colors.text_muted
                        } else {
                            colors.text
                        })
                        .child(display),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child("Edit"),
                ),
        )
}

fn remote_hostname_block(
    current: &str,
    edit: Option<(&(String, String, String), bool)>,
    caret_on: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    if let Some((parts, empty)) = edit {
        return remote_field_edit_row(
            FieldEditPaint {
                id: "remote-hostname-edit",
                label: "Hostname",
                parts,
                empty,
                placeholder: "e.g. macbook.tailnet.ts.net",
                caret_on,
            },
            cx,
        )
        .into_any_element();
    }
    let current_owned = current.to_string();
    remote_field_static_row(
        FieldStaticPaint {
            id: "remote-hostname",
            label: "Hostname",
            empty_hint: "(optional — Tailscale MagicDNS / LAN name)",
        },
        current,
        move |this, window, cx| {
            this.begin_hostname_edit(current_owned.clone(), window, cx);
        },
        cx,
    )
    .into_any_element()
}

fn remote_password_block(
    current: &str,
    edit: Option<(&(String, String, String), bool)>,
    caret_on: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    if let Some((parts, empty)) = edit {
        return remote_field_edit_row(
            FieldEditPaint {
                id: "remote-password-edit",
                label: "Password",
                parts,
                empty,
                placeholder: "Enter password…",
                caret_on,
            },
            cx,
        )
        .into_any_element();
    }
    let current_owned = current.to_string();
    remote_field_static_row(
        FieldStaticPaint {
            id: "remote-password",
            label: "Password",
            empty_hint: "(not set — generated on first enable)",
        },
        current,
        move |this, window, cx| {
            this.begin_password_edit(current_owned.clone(), window, cx);
        },
        cx,
    )
    .into_any_element()
}

/// Mono value row; click copies the full string (GPUI text is not OS-selectable).
fn copyable_mono_row(
    id: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let value = value.into();
    let value_for_copy = value.clone();
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .rounded_sm()
        .px_2()
        .py_1()
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .on_click(cx.listener(move |_, _, window, cx| {
            cx.stop_propagation();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(value_for_copy.to_string()));
            window.refresh();
            cx.notify();
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_sm()
                .font_family("Menlo")
                .text_color(colors.text)
                .child(value),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(colors.text_muted)
                .child("Copy"),
        )
}

fn remote_toggle_row(
    enabled: bool,
    subtitle: &'static str,
    focused: bool,
    cx: &mut Context<SettingsView>,
) -> impl IntoElement {
    let colors = cx.theme().colors().clone();
    let background = if enabled {
        colors.element_selected
    } else {
        colors.elevated_surface_background
    };
    let row_bg = if focused {
        colors.element_hover
    } else {
        gpui::transparent_black()
    };
    div()
        .id("mobile-remote-toggle")
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .bg(row_bg)
        .cursor_pointer()
        .hover(|s| s.bg(colors.element_hover))
        .on_click(cx.listener(|_, _, window, cx| {
            cx.stop_propagation();
            toggle_mobile_remote_from_settings(window, cx);
            window.refresh();
            cx.notify();
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child("Mobile remote"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .flex_none()
                .w(px(18.))
                .h(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(colors.border)
                .bg(background)
                .text_xs()
                .children(enabled.then_some("x")),
        )
}

fn toggle_mobile_remote_from_settings(window: &mut gpui::Window, cx: &mut App) {
    use crate::app::remote::MainApp;
    let Some(main) = cx.try_global::<MainApp>().map(|m| m.0.clone()) else {
        log::warn!("mobile remote: main app handle missing");
        return;
    };
    let _ = main.update(cx, |app, cx| {
        app.toggle_mobile_remote_quiet(window, cx);
    });
}

/// Keyboard activation of the Mobile remote toggle (Settings focus index 2).
pub(super) fn activate_mobile_remote(window: &mut gpui::Window, cx: &mut App) {
    toggle_mobile_remote_from_settings(window, cx);
}
