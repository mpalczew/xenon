//! Keep the machine from idle-sleeping while the phone remote is on. macOS:
//! `caffeinate -i -w <our pid>` (also exits if Xenon dies); closing the lid
//! still sleeps the Mac. Linux: a `systemd-inhibit ... tail --pid` child that
//! holds an idle inhibitor lock until Xenon exits or the child is killed.

use std::process::{Child, Command, Stdio};

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
        "--why=Phone remote is on",
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
