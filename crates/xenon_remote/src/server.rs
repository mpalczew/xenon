//! HTTP + WebSocket server for the mobile remote. One OS thread per
//! connection; everything stateful lives in the host behind `HostRequest`.

use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::host::{ConnId, HostRequest, HostTx, PairError};
use crate::http;
use crate::protocol::{PairRequest, PairResponse, TerminalInfo, WorkspaceInfo};
use crate::rate_limit::RateLimiter;
use crate::uploads;
use crate::ws;

const PAGE: &str = include_str!("page.html");
const MANIFEST: &str = include_str!("../assets/manifest.webmanifest");
const ICON_180: &[u8] = include_bytes!("../assets/icon-180.png");
const ICON_192: &[u8] = include_bytes!("../assets/icon-192.png");
const ICON_512: &[u8] = include_bytes!("../assets/icon-512.png");
const UPLOAD_PATH: &str = "/api/uploads";
const HOST_TIMEOUT: Duration = Duration::from_secs(5);
/// Concurrent connections (phones hold one socket; the rest are short requests).
const MAX_CONNECTIONS: usize = 32;
/// How often a listener checks for shutdown between accepts.
const ACCEPT_POLL: Duration = Duration::from_millis(50);

/// State shared by every connection thread.
pub(crate) struct Shared {
    pub(crate) host: HostTx,
    pub(crate) enabled: AtomicBool,
    pub(crate) limiter: Mutex<RateLimiter>,
    /// Where phone images land; uploads are refused until set.
    pub(crate) uploads_dir: OnceLock<PathBuf>,
    next_conn: AtomicU64,
    active: AtomicUsize,
}

impl Shared {
    pub(crate) fn next_conn(&self) -> ConnId {
        ConnId(self.next_conn.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn running(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub(crate) fn allow(&self, ip: IpAddr) -> bool {
        self.limiter
            .lock()
            .expect("limiter lock")
            .allow(ip, Instant::now())
    }

    pub(crate) fn fail(&self, ip: IpAddr) {
        self.limiter
            .lock()
            .expect("limiter lock")
            .record_failure(ip, Instant::now());
    }

    /// Device token → device id via the host. `Err` = the host is busy or
    /// gone (not a verdict on the token: callers must not sign the phone out).
    pub(crate) fn authenticate(&self, token: &str) -> Result<Option<String>, String> {
        if token.is_empty() {
            return Ok(None);
        }
        ask(&self.host, |reply| HostRequest::Authenticate {
            token: token.to_string(),
            reply,
        })
    }
}

/// Why the server did not start.
#[derive(Debug)]
pub enum StartError {
    /// Another process (typically the other a/b Xenon slot) holds the port.
    PortBusy,
    Other(anyhow::Error),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PortBusy => write!(f, "the port is in use by another app"),
            Self::Other(e) => write!(f, "{e:#}"),
        }
    }
}

/// Running mobile remote server. Drop stops accepting and waits until every
/// port is released, so a restart (or the other slot) can bind it at once.
pub struct RemoteServer {
    pub port: u16,
    /// Addresses actually bound (e.g. Tailscale IP + loopback).
    pub binds: Vec<SocketAddr>,
    shared: Arc<Shared>,
    accepts: Vec<thread::JoinHandle<()>>,
}

impl RemoteServer {
    /// Listen on `port` at each of `ips` (port 0 = ephemeral; use one ip then).
    /// All-or-nothing on a busy port; other per-address failures are skipped
    /// (e.g. an interface that just went away) as long as one address binds.
    pub fn start(ips: &[IpAddr], port: u16, host: HostTx) -> Result<Self, StartError> {
        let listeners = bind_all(ips, port)?;
        let shared = Arc::new(Shared {
            host,
            enabled: AtomicBool::new(true),
            limiter: Mutex::new(RateLimiter::new()),
            uploads_dir: OnceLock::new(),
            next_conn: AtomicU64::new(1),
            active: AtomicUsize::new(0),
        });
        let mut binds = Vec::new();
        let mut accepts = Vec::new();
        for listener in listeners {
            let (addr, handle) = spawn_accept(listener, &shared).map_err(StartError::Other)?;
            binds.push(addr);
            accepts.push(handle);
        }
        Ok(Self {
            port: binds[0].port(),
            binds,
            shared,
            accepts,
        })
    }

    /// Enable image uploads, saved under `dir`. First call wins.
    pub fn set_uploads_dir(&self, dir: PathBuf) {
        let _ = self.shared.uploads_dir.set(dir);
    }

    /// Listeners notice within `ACCEPT_POLL` and release their ports.
    pub fn stop(&self) {
        self.shared.enabled.store(false, Ordering::SeqCst);
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.stop();
        for handle in self.accepts.drain(..) {
            let _ = handle.join();
        }
    }
}

fn bind_all(ips: &[IpAddr], port: u16) -> Result<Vec<TcpListener>, StartError> {
    let mut listeners = Vec::new();
    for ip in ips {
        match TcpListener::bind((*ip, port)) {
            Ok(listener) => listeners.push(listener),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                return Err(StartError::PortBusy);
            }
            Err(e) => log::warn!("xenon remote: bind {ip}:{port}: {e}"),
        }
    }
    if listeners.is_empty() {
        return Err(StartError::Other(anyhow!(
            "could not listen on port {port}"
        )));
    }
    Ok(listeners)
}

fn spawn_accept(
    listener: TcpListener,
    shared: &Arc<Shared>,
) -> Result<(SocketAddr, thread::JoinHandle<()>)> {
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let shared = shared.clone();
    let handle = thread::Builder::new()
        .name(format!("xenon-remote-{addr}"))
        .spawn(move || accept_loop(listener, shared))?;
    log::info!("xenon remote listening on {addr}");
    Ok((addr, handle))
}

fn accept_loop(listener: TcpListener, shared: Arc<Shared>) {
    while shared.running() {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL);
                continue;
            }
            Err(_) => continue,
        };
        if shared.active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            shared.active.fetch_sub(1, Ordering::SeqCst);
            continue; // Dropped: too many open connections.
        }
        let shared = shared.clone();
        thread::spawn(move || {
            let _ = stream.set_nonblocking(false);
            if let Err(e) = handle_connection(stream, &shared) {
                log::debug!("xenon remote connection: {e:#}");
            }
            shared.active.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

/// A parsed request head plus body.
pub(crate) struct Request {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: Vec<u8>,
    pub(crate) peer: IpAddr,
}

fn read_request(stream: &mut TcpStream, shared: &Shared) -> Result<Option<Request>> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let peer = stream.peer_addr()?.ip();
    let mut buf = vec![0u8; 16 * 1024];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    let head = String::from_utf8_lossy(&buf[..n]).into_owned();
    let (req_line, headers, body_start) = http::split_http(&head)?;
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let (path, _query) = http::split_path_query(parts.next().unwrap_or("/"));
    let path = path.to_string();
    let clen = http::content_length(&headers);
    let upload = method == "POST" && path == UPLOAD_PATH && signed_in(&headers, shared);
    let limit = if upload {
        uploads::MAX_UPLOAD
    } else {
        http::MAX_BODY
    };
    if clen > limit {
        http::write_json_error(stream, 413, "too large")?;
        return Ok(None);
    }
    let body = http::extract_body(&buf[..n], body_start, clen, stream)?;
    Ok(Some(Request {
        method,
        path,
        headers,
        body,
        peer,
    }))
}

/// Only a signed-in phone may send an image-sized body.
fn signed_in(headers: &HashMap<String, String>, shared: &Shared) -> bool {
    matches!(shared.authenticate(bearer(headers)), Ok(Some(_)))
}

fn handle_connection(mut stream: TcpStream, shared: &Shared) -> Result<()> {
    let Some(req) = read_request(&mut stream, shared)? else {
        return Ok(());
    };
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/" | "/index.html" | "/pair") => http::write_http(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            PAGE.as_bytes(),
        ),
        ("GET", "/manifest.webmanifest") => http::write_http(
            &mut stream,
            200,
            "application/manifest+json",
            MANIFEST.as_bytes(),
        ),
        ("GET", "/icon-180.png") => http::write_http(&mut stream, 200, "image/png", ICON_180),
        ("GET", "/icon-192.png") => http::write_http(&mut stream, 200, "image/png", ICON_192),
        ("GET", "/icon-512.png") => http::write_http(&mut stream, 200, "image/png", ICON_512),
        ("POST", "/pair") => pair(&mut stream, &req, shared),
        ("GET", "/ws") => ws::serve(stream, &req, shared),
        ("POST", UPLOAD_PATH) => uploads::serve(&mut stream, &req, shared),
        ("GET", p) if p.starts_with("/api/") => api(&mut stream, &req, shared),
        _ => http::write_http(&mut stream, 404, "text/plain", b"not found"),
    }
}

fn pair(stream: &mut TcpStream, req: &Request, shared: &Shared) -> Result<()> {
    if !shared.allow(req.peer) {
        return http::write_json_error(stream, 429, "too many attempts");
    }
    let Ok(body) = serde_json::from_slice::<PairRequest>(&req.body) else {
        return http::write_json_error(stream, 400, "bad body");
    };
    let result: Result<PairResponse, PairError> = ask(&shared.host, |reply| HostRequest::Pair {
        code: body.code,
        label: body.label,
        reply,
    })
    .unwrap_or_else(|e| Err(PairError::Storage(e)));
    match result {
        Ok(resp) => {
            let bytes = serde_json::to_vec(&resp)?;
            http::write_http(stream, 200, "application/json", &bytes)
        }
        Err(PairError::Rejected) => {
            shared.fail(req.peer);
            http::write_json_error(stream, 401, "code not recognized")
        }
        Err(PairError::Storage(e)) => http::write_json_error(stream, 500, &e),
    }
}

/// Device tokens are 256-bit: no throttle here, so a stranger's bad guesses
/// can never lock out paired phones.
fn api(stream: &mut TcpStream, req: &Request, shared: &Shared) -> Result<()> {
    match shared.authenticate(bearer(&req.headers)) {
        Ok(Some(_)) => {}
        Ok(None) => return http::write_json_error(stream, 401, "unauthorized"),
        Err(e) => return http::write_json_error(stream, 503, &e),
    }
    if req.path == "/api/workspaces" {
        let list: Vec<WorkspaceInfo> =
            ask(&shared.host, |reply| HostRequest::ListWorkspaces { reply })
                .map_err(|e| anyhow!(e))?;
        return write_json(stream, &list);
    }
    let workspace = req
        .path
        .strip_prefix("/api/workspaces/")
        .and_then(|s| s.strip_suffix("/terminals"))
        .map(http::urlencoding_decode);
    let Some(workspace_id) = workspace else {
        return http::write_json_error(stream, 404, "not found");
    };
    let terminals: Result<Vec<TerminalInfo>, String> =
        ask(&shared.host, |reply| HostRequest::ListTerminals {
            workspace_id,
            reply,
        })
        .and_then(|r| r);
    match terminals {
        Ok(list) => write_json(stream, &list),
        Err(e) => http::write_json_error(stream, 404, &e),
    }
}

fn write_json<T: serde::Serialize>(stream: &mut TcpStream, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    http::write_http(stream, 200, "application/json", &bytes)
}

pub(crate) fn bearer(headers: &HashMap<String, String>) -> &str {
    headers
        .get("authorization")
        .map(String::as_str)
        .and_then(|a| {
            a.strip_prefix("Bearer ")
                .or_else(|| a.strip_prefix("bearer "))
        })
        .unwrap_or("")
        .trim()
}

/// Send one request to the host and wait for its reply.
pub(crate) fn ask<T>(
    host: &HostTx,
    make: impl FnOnce(mpsc::SyncSender<T>) -> HostRequest,
) -> Result<T, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    host.send_blocking(make(tx))
        .map_err(|_| "host gone".to_string())?;
    rx.recv_timeout(HOST_TIMEOUT)
        .map_err(|_| "host timeout".to_string())
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod http_tests;
