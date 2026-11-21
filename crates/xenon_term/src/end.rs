use std::io;
use std::process::Command;

/// End every terminal, including wedged holders and holders whose protocol this build
/// no longer speaks. Each unit in the slice gets a hangup, then SIGKILL after 5 seconds:
/// what closing a terminal does, applied by the platform. Idempotent.
pub fn end_all() -> io::Result<()> {
    let output = Command::new("systemctl")
        .args(["--user", "stop", &crate::start::slice()])
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(io::Error::other(format!("systemctl stop: {text}")))
}
