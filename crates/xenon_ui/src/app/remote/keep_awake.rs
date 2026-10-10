//! Keep the machine from idle-sleeping while a phone is connected, if the user
//! opted in. macOS:
//! `caffeinate -i -w <our pid>` (also exits if Xenon dies); closing the lid
//! still sleeps the Mac. Linux: a `systemd-inhibit ... tail --pid` child that
//! holds an idle inhibitor lock until Xenon exits or the child is killed.

use std::process::{Child, Command, Stdio};

use super::*;

/// Sheet and Settings wording; names this machine as the platform does.
#[cfg(target_os = "macos")]
pub(crate) const KEEP_AWAKE_LABEL: &str = "Keep this Mac awake while a phone is connected";
#[cfg(not(target_os = "macos"))]
pub(crate) const KEEP_AWAKE_LABEL: &str = "Keep this computer awake while a phone is connected";
pub(crate) const KEEP_AWAKE_DETAIL: &str = "Ignores your sleep setting until the phone disconnects";

/// The inhibitor is held exactly while the remote is on, the user opted in,
/// and at least one phone is connected.
pub(crate) fn should_hold(remote_on: bool, opted_in: bool, connected: usize) -> bool {
    remote_on && opted_in && connected > 0
}

/// Whether the idle-sleep inhibitor is held. Dropping it releases.
#[derive(Default)]
pub(crate) enum Inhibitor {
    #[default]
    Released,
    Held {
        _guard: KeepAwake,
    },
}

impl Inhibitor {
    /// Acquire or release so that `hold` is true afterwards (if possible).
    pub(crate) fn sync(&mut self, hold: bool) {
        match (hold, &*self) {
            (true, Self::Released) => {
                if let Some(guard) = KeepAwake::acquire() {
                    *self = Self::Held { _guard: guard };
                }
            }
            (false, Self::Held { .. }) => *self = Self::Released,
            _ => {}
        }
    }
}

pub(crate) struct KeepAwake {
    child: Child,
}

impl KeepAwake {
    pub(crate) fn acquire() -> Option<Self> {
        let child = keep_awake_command()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| log::warn!("keep-awake unavailable: {e}"))
            .ok()?;
        Some(Self { child })
    }
}

#[cfg(target_os = "macos")]
fn keep_awake_command() -> Command {
    let mut command = Command::new("/usr/bin/caffeinate");
    command.args(["-i", "-w", &std::process::id().to_string()]);
    command
}

#[cfg(not(target_os = "macos"))]
fn keep_awake_command() -> Command {
    let mut command = Command::new("systemd-inhibit");
    command.args([
        "--what=idle",
        "--who=Xenon",
        "--why=A phone is connected to Xenon",
        "--mode=block",
        "tail",
        "--pid",
        &std::process::id().to_string(),
        "-f",
        "/dev/null",
    ]);
    command
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        // Our own child only.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl XenonApp {
    /// Make the inhibitor match the snapshot just published.
    pub(super) fn sync_keep_awake(&mut self, info: &MobileRemoteInfo) {
        let hold = should_hold(info.enabled, info.keep_awake, info.connected);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_only_when_on_opted_in_and_a_phone_is_connected() {
        for remote_on in [false, true] {
            for opted_in in [false, true] {
                for connected in [0, 1, 3] {
                    let want = remote_on && opted_in && connected > 0;
                    assert_eq!(should_hold(remote_on, opted_in, connected), want);
                }
            }
        }
        assert!(should_hold(true, true, 1));
        assert!(!should_hold(true, true, 0));
        assert!(!should_hold(true, false, 2));
        assert!(!should_hold(false, true, 2));
    }

    #[test]
    fn releasing_needs_no_process() {
        let mut inhibitor = Inhibitor::default();
        inhibitor.sync(false);
        assert!(matches!(inhibitor, Inhibitor::Released));
    }
}
