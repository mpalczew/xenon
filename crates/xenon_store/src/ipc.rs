//! Single-instance IPC: one Unix socket per data dir (`ipc.sock`).
//!
//! A second process with open paths connects, sends a request, and exits.
//! Socket path follows `data_dir()` so A/B slots stay isolated; ship uses one
//! `~/.xenon` socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::data_dir;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(400);
const IO_TIMEOUT: Duration = Duration::from_millis(2_000);

/// Path of the per-data-dir control socket.
pub fn socket_path() -> PathBuf {
    data_dir().join("ipc.sock")
}

/// Where an agent-originated file should land.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OpenPane {
    #[default]
    Sibling,
    Focused,
    SplitRight,
}

impl OpenPane {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw {
            "sibling" => Some(Self::Sibling),
            "focused" => Some(Self::Focused),
            "split-right" => Some(Self::SplitRight),
            _ => None,
        }
    }
}

/// One file to open, with optional 1-based location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenFileSpec {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(default)]
    pub pane: OpenPane,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Open zero or more paths (dirs = workspaces, files = editors).
    Open { paths: Vec<PathBuf> },
    /// Open one file at a location, in a chosen pane. Does not steal focus.
    OpenFile(OpenFileSpec),
    /// Focus the running window without opening anything.
    Activate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// If a live primary is listening, deliver `paths` (or Activate) and return true.
/// False means this process should become primary (or cold-start).
pub fn try_handoff(paths: &[PathBuf]) -> bool {
    let req = if paths.is_empty() {
        IpcRequest::Activate
    } else {
        IpcRequest::Open {
            paths: paths.to_vec(),
        }
    };
    try_handoff_request(&req)
}

/// Same as [`try_handoff`] for an already-built request (`open` included).
pub fn try_handoff_request(req: &IpcRequest) -> bool {
    send_request(req).is_ok()
}

/// Connect and send one request; Ok only when the peer replies `ok: true`.
pub fn send_request(req: &IpcRequest) -> std::io::Result<IpcResponse> {
    let path = socket_path();
    let mut stream = connect_socket(&path)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let line = serde_json::to_string(req)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line)?;
    let resp: IpcResponse = serde_json::from_str(resp_line.trim()).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("bad ipc response: {e}"),
        )
    })?;
    if resp.ok {
        Ok(resp)
    } else {
        Err(std::io::Error::other(
            resp.error.unwrap_or_else(|| "ipc rejected".into()),
        ))
    }
}

fn connect_socket(path: &Path) -> std::io::Result<UnixStream> {
    // `connect_timeout` is for IP sockets; Unix uses plain connect + short fail.
    let _ = CONNECT_TIMEOUT;
    UnixStream::connect(path)
}

/// Bind as primary. Removes a stale socket file first.
pub fn bind_server() -> std::io::Result<UnixListener> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let path = socket_path();
    if path.exists() {
        // Stale file from a crashed primary — remove and rebind.
        let _ = std::fs::remove_file(&path);
    }
    UnixListener::bind(&path)
}

/// Serve until the listener is dropped / errors. Each request is pushed to `tx`.
pub fn serve_forever(listener: UnixListener, tx: impl Fn(IpcRequest) + Send + 'static) {
    std::thread::Builder::new()
        .name("xenon-ipc".into())
        .spawn(move || {
            for conn in listener.incoming() {
                let Ok(stream) = conn else {
                    continue;
                };
                if let Err(error) = handle_client(stream, &tx) {
                    log::debug!("ipc client: {error}");
                }
            }
            // Clean up socket when the listener loop ends.
            let _ = std::fs::remove_file(socket_path());
        })
        .expect("spawn xenon-ipc thread");
}

fn handle_client(stream: UnixStream, tx: &impl Fn(IpcRequest)) -> std::io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let req: IpcRequest = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            write_response(
                &stream,
                IpcResponse {
                    ok: false,
                    error: Some(format!("bad request: {e}")),
                },
            )?;
            return Ok(());
        }
    };
    tx(req);
    write_response(
        &stream,
        IpcResponse {
            ok: true,
            error: None,
        },
    )
}

fn write_response(mut stream: &UnixStream, resp: IpcResponse) -> std::io::Result<()> {
    let line = serde_json::to_string(&resp)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

/// Expand CLI args into absolute paths (cwd-relative). Skips flags.
pub fn parse_cli_paths(args: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for arg in args {
        if arg == "--" {
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        let path = PathBuf::from(&arg);
        let abs = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        // Prefer canonicalize when the path exists; keep joined path otherwise.
        out.push(abs.canonicalize().unwrap_or(abs));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cli_skips_flags() {
        let paths = parse_cli_paths(
            ["--help", "rel", "/abs"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
        );
        assert_eq!(paths.len(), 2);
        assert!(paths[1].is_absolute());
    }
}
