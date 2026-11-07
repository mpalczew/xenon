//! Keep the machine from idle-sleeping while a phone is connected, if the user
//! opted in. A `systemd-inhibit ... tail --pid` child holds an idle inhibitor
//! lock until Xenon exits or the child is killed.

use std::process::{Child, Command, Stdio};

/// Sheet and Settings wording.
pub const KEEP_AWAKE_LABEL: &str = "Keep this computer awake while a phone is connected";
pub const KEEP_AWAKE_DETAIL: &str = "Ignores your sleep setting until the phone disconnects";

/// The inhibitor is held exactly while the remote is on, the user opted in,
/// and at least one phone is connected.
pub fn should_hold(remote_on: bool, opted_in: bool, connected: usize) -> bool {
    remote_on && opted_in && connected > 0
}

/// Whether the idle-sleep inhibitor is held. Dropping it releases.
#[derive(Default)]
pub enum Inhibitor {
    #[default]
    Released,
    Held {
        _guard: KeepAwake,
    },
}

impl Inhibitor {
    /// Acquire or release so that `hold` is true afterwards (if possible).
    pub fn sync(&mut self, hold: bool) {
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

pub struct KeepAwake {
    child: Child,
}

impl KeepAwake {
    fn acquire() -> Option<Self> {
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
        let _ = self.child.kill();
        let _ = self.child.wait();
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
}
