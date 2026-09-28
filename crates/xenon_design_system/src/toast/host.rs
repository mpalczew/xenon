//! One toast host per window, so any crate can report a result.

use gpui::{AnyWindowHandle, App, AppContext, Entity, Global, Pixels, WeakEntity, Window};

use super::{Toast, ToastView};

/// Every window's toast host, oldest first. The oldest live one is the app's
/// primary host, used when a caller has no window at hand.
#[derive(Default)]
struct ToastHosts(Vec<(AnyWindowHandle, WeakEntity<ToastView>)>);

impl Global for ToastHosts {}

/// Creates `window`'s toast host. Render it inside a `relative()` container.
pub fn toast_host(top: Pixels, window: &Window, cx: &mut App) -> Entity<ToastView> {
    let view = cx.new(|cx| ToastView::new(top, cx));
    let hosts = cx.default_global::<ToastHosts>();
    hosts.0.retain(|(_, host)| host.upgrade().is_some());
    hosts.0.push((window.window_handle(), view.downgrade()));
    view
}

/// Shows `toast` in the app's primary window.
pub fn show_toast(toast: Toast, cx: &mut App) {
    let host = cx
        .try_global::<ToastHosts>()
        .and_then(|hosts| hosts.0.iter().find_map(|(_, host)| host.upgrade()));
    if let Some(host) = host {
        host.update(cx, |view, cx| view.show(toast, cx));
    }
}

/// Shows `toast` in `window`, or in the primary window when `window` has no host.
pub fn show_toast_in(window: &Window, toast: Toast, cx: &mut App) {
    let handle = window.window_handle();
    let host = cx.try_global::<ToastHosts>().and_then(|hosts| {
        hosts
            .0
            .iter()
            .find(|(owner, _)| *owner == handle)
            .and_then(|(_, host)| host.upgrade())
    });
    match host {
        Some(host) => host.update(cx, |view, cx| view.show(toast, cx)),
        None => show_toast(toast, cx),
    }
}
