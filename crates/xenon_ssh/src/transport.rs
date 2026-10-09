use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub(crate) const HELPER: &str = include_str!("helper.py");

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn request(host: &str, request: &Value) -> Result<Value> {
    let control = control_path()?;
    let mut child = Command::new("ssh")
        .args(["-o", "ControlMaster=auto", "-o", "ControlPersist=60", "-o"])
        .arg(format!("ControlPath={}", control.display()))
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=10",
            "-o",
            "ServerAliveCountMax=2",
            "--",
            host,
        ])
        .arg(format!("python3 -c {}", quote(HELPER)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Couldn’t start SSH")?;
    let mut input = child.stdin.take().context("SSH stdin unavailable")?;
    serde_json::to_writer(&mut input, request)?;
    input.write_all(b"\n")?;
    drop(input);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!("SSH: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    decode(&output.stdout)
}

pub(crate) fn control_path() -> Result<PathBuf> {
    let home =
        std::env::var_os("HOME").context("HOME is unavailable for SSH connection sharing")?;
    let directory = PathBuf::from(home).join(".xenon/ssh");
    std::fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(directory.join("%C"))
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Value> {
    let response: Value = serde_json::from_slice(bytes).context("Invalid SSH helper response")?;
    if let Some(error) = response.get("error").and_then(Value::as_str) {
        bail!("{error}");
    }
    response
        .get("value")
        .cloned()
        .context("Missing SSH helper response")
}
