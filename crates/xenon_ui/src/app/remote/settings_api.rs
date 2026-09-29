//! Entry points for the Settings window (a separate window): each reaches the
//! main app through `MainApp` and persists the choice.

use super::*;

pub(crate) fn with_main(cx: &mut App, f: impl FnOnce(&mut XenonApp, &mut Context<XenonApp>)) {
    let Some(main) = cx.try_global::<MainApp>().map(|m| m.0.clone()) else {
        log::warn!("phone remote: main app handle missing");
        return;
    };
    let _ = main.update(cx, f);
}

pub(crate) fn settings_toggle_remote(cx: &mut App) {
    with_main(cx, |app, cx| app.toggle_mobile_remote(cx));
}

pub(crate) fn settings_set_network(network: RemoteNetwork, cx: &mut App) {
    if let Err(e) = xenon_store::update_settings(|s| s.remote_network = network) {
        log::warn!("save remote network: {e}");
    }
    with_main(cx, |app, cx| {
        app.restart_mobile_remote(cx);
        app.publish_remote_info(cx);
    });
}

pub(crate) fn settings_toggle_keep_awake(cx: &mut App) {
    if let Err(e) = xenon_store::update_settings(|s| s.remote_keep_awake = !s.remote_keep_awake) {
        log::warn!("save remote keep-awake: {e}");
    }
    with_main(cx, |app, cx| {
        let on = xenon_store::load_settings()
            .map(|s| s.remote_keep_awake)
            .unwrap_or(true);
        if let Some(runtime) = app.services.remote.as_mut() {
            runtime._keep_awake = on.then(KeepAwake::acquire).flatten();
        }
        app.publish_remote_info(cx);
    });
}

/// Persist the phone URL host override (no restart: bind is unaffected).
pub(crate) fn set_remote_hostname(hostname: String, window: &Window, cx: &mut App) {
    let hostname = normalize_hostname(&hostname);
    if let Err(e) = xenon_store::update_settings(|s| s.remote_hostname = hostname.clone()) {
        let toast = toasts::failed("Couldn’t save the hostname", e);
        xenon_design_system::show_toast_in(window, toast, cx);
        return;
    }
    with_main(cx, |app, cx| app.publish_remote_info(cx));
}

pub(crate) fn settings_revoke_device(id: String, cx: &mut App) {
    with_main(cx, |app, cx| app.revoke_remote_devices(Some(&id), cx));
}

pub(crate) fn settings_revoke_all(cx: &mut App) {
    with_main(cx, |app, cx| app.revoke_remote_devices(None, cx));
}

/// Settings "Connect Phone…": bring the main window forward with the sheet.
pub(crate) fn settings_connect_phone(cx: &mut App) {
    with_main(cx, |app, cx| {
        if let Some(main) = app.services.main_window {
            let _ = main.update(cx, |_, window, _| window.activate_window());
        }
        app.open_connect_phone(cx);
    });
}

impl XenonApp {
    /// Forget one device (or all) on disk and close their live sessions.
    fn revoke_remote_devices(&mut self, id: Option<&str>, cx: &mut Context<Self>) {
        let mut book = self
            .services
            .remote
            .as_mut()
            .map(|r| {
                std::mem::replace(&mut r.devices, DeviceBook::from_devices(Default::default()))
            })
            .unwrap_or_else(DeviceBook::load);
        book.reload();
        match id {
            Some(id) => {
                book.revoke(id);
            }
            None => book.revoke_all(),
        }
        if let Err(e) = book.save() {
            self.show_toast(toasts::failed("Couldn’t update paired devices", e), cx);
        }
        if let Some(runtime) = self.services.remote.as_mut() {
            runtime.devices = book;
        }
        self.drop_device_connections(id, cx);
        self.publish_remote_info(cx);
    }
}
