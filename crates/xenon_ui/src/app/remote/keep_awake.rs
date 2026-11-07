//! Keep-awake while a phone is connected: the hold itself is `xenon_remote`'s
//! `Inhibitor`; this applies the published status and flips the opt-in.

use super::*;

impl XenonApp {
    /// Make the inhibitor match the snapshot just published.
    pub(super) fn sync_keep_awake(&mut self, info: &MobileRemoteInfo) {
        let hold = xenon_remote::should_hold(info.enabled, info.keep_awake, info.connected);
        if let Some(runtime) = self.services.remote.as_mut() {
            runtime.keep_awake.sync(hold);
        }
    }

    /// Flip the opt-in (sheet key or Settings switch) and apply it now.
    pub(crate) fn toggle_keep_awake(&mut self, cx: &mut Context<Self>) {
        let update = |s: &mut xenon_store::AppSettings| {
            s.remote_keep_awake_while_connected = !s.remote_keep_awake_while_connected;
        };
        if let Err(e) = xenon_store::update_settings(update) {
            log::warn!("save remote keep-awake: {e}");
        }
        self.publish_remote_info(cx);
    }
}
