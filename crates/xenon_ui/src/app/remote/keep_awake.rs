//! Keep the Mac from idle-sleeping while the phone remote is on, via a
//! `caffeinate -i -w <our pid>` child: it also exits if Xenon dies.
//! Closing the lid still sleeps the Mac (macOS policy).

use std::process::{Child, Command, Stdio};

pub(crate) struct KeepAwake {
    child: Child,
}

impl KeepAwake {
    pub(crate) fn acquire() -> Option<Self> {
        let child = Command::new("/usr/bin/caffeinate")
            .args(["-i", "-w", &std::process::id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| log::warn!("keep-awake unavailable: {e}"))
            .ok()?;
        Some(Self { child })
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        // Our own child only.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
