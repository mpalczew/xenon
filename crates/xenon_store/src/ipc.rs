use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::data_dir;

const IO_TIMEOUT: Duration = Duration::from_millis(2_000);

pub(crate) fn socket_path() -> PathBuf {
    data_dir().join("ipc.sock")
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum IpcRequest {
    Open { paths: Vec<PathBuf> },
    OpenFile(OpenFileSpec),
    Activate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct IpcResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub enum Handoff {
    Sent,
    Rejected(String),
    NoServer,
}

pub fn try_handoff_request(req: &IpcRequest) -> Handoff {
    match send_request(req) {
        Ok(_) => Handoff::Sent,
        Err(error) if error.kind() == std::io::ErrorKind::Other => {
            Handoff::Rejected(error.to_string())
        }
        Err(_) => Handoff::NoServer,
    }
}

/// Connect and send one request; Ok only when the peer replies `ok: true`.
pub(crate) fn send_request(req: &IpcRequest) -> std::io::Result<IpcResponse> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)?;
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

/// Bind as primary. Removes a stale socket file first.
pub fn bind_server() -> std::io::Result<UnixListener> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let path = socket_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    UnixListener::bind(&path)
}

pub fn serve_forever(
    listener: UnixListener,
    tx: impl Fn(IpcRequest) -> Result<(), String> + Send + 'static,
) {
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
            let _ = std::fs::remove_file(socket_path());
        })
        .expect("spawn xenon-ipc thread");
}

fn handle_client(
    stream: UnixStream,
    tx: &impl Fn(IpcRequest) -> Result<(), String>,
) -> std::io::Result<()> {
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
    let error = tx(req).err();
    write_response(
        &stream,
        IpcResponse {
            ok: error.is_none(),
            error,
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
pub(crate) fn parse_cli_paths(args: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
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
        out.push(abs.canonicalize().unwrap_or(abs));
    }
    out
}
