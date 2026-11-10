use std::process::{Child, Command, Stdio};

pub const KEEP_AWAKE_LABEL: &str = "Keep this computer awake while a phone is connected";
pub const KEEP_AWAKE_DETAIL: &str = "Ignores your sleep setting until the phone disconnects";

pub fn should_hold(remote_on: bool, opted_in: bool, connected: usize) -> bool {
    remote_on && opted_in && connected > 0
}

#[derive(Default)]
pub enum Inhibitor {
    #[default]
    Released,
    Held {
        _guard: KeepAwake,
    },
}

impl Inhibitor {
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
