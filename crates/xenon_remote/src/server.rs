use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use percent_encoding::percent_decode_str;

use crate::host::{ConnId, HostRequest, HostTx, PairError};
use crate::http::{self, Request};
use crate::protocol::{PairRequest, PairResponse};
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
const MAX_CONNECTIONS: usize = 32;
const ACCEPT_POLL: Duration = Duration::from_millis(250);

pub(crate) struct Shared {
    pub(crate) host: HostTx,
    pub(crate) enabled: AtomicBool,
    pub(crate) limiter: Mutex<RateLimiter>,
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

#[derive(Debug)]
pub enum StartError {
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

pub struct RemoteServer {
    pub port: u16,
    pub binds: Vec<SocketAddr>,
    shared: Arc<Shared>,
    accepts: Vec<thread::JoinHandle<()>>,
}

impl RemoteServer {
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

    pub fn set_uploads_dir(&self, dir: PathBuf) {
        let _ = self.shared.uploads_dir.set(dir);
    }

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
            continue;
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

fn read_request(stream: &mut TcpStream, shared: &Shared) -> Result<Option<Request>> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let peer = stream.peer_addr()?.ip();
    let Some(mut req) = http::read_head(stream, peer)? else {
        return Ok(None);
    };
    let upload = req.method == ::http::Method::POST
        && req.path == UPLOAD_PATH
        && matches!(shared.authenticate(bearer(&req)), Ok(Some(_)));
    let limit = if upload {
        uploads::MAX_UPLOAD
    } else {
        http::MAX_BODY
    };
    if req.content_length() > limit {
        http::write_json_error(stream, 413, "too large")?;
        return Ok(None);
    }
    http::read_body(&mut req, stream)?;
    Ok(Some(req))
}

fn handle_connection(mut stream: TcpStream, shared: &Shared) -> Result<()> {
    let Some(req) = read_request(&mut stream, shared)? else {
        return Ok(());
    };
    let html = "text/html; charset=utf-8";
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/" | "/index.html" | "/pair") => {
            http::write_http(&mut stream, 200, html, PAGE.as_bytes())
        }
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
        ("GET" | "POST" | "DELETE", p) if p.starts_with("/api/") => api(&mut stream, &req, shared),
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
        Ok(resp) => http::write_json(stream, &resp),
        Err(PairError::Rejected) => {
            shared.fail(req.peer);
            http::write_json_error(stream, 401, "code not recognized")
        }
        Err(PairError::Storage(e)) => http::write_json_error(stream, 500, &e),
    }
}

fn api(stream: &mut TcpStream, req: &Request, shared: &Shared) -> Result<()> {
    match shared.authenticate(bearer(req)) {
        Ok(Some(_)) => {}
        Ok(None) => return http::write_json_error(stream, 401, "unauthorized"),
        Err(e) => return http::write_json_error(stream, 503, &e),
    }
    let host = &shared.host;
    let segments: Vec<&str> = req.path.split('/').skip(1).collect();
    match (req.method.as_str(), segments.as_slice()) {
        ("GET", ["api", "workspaces"]) => reply(
            stream,
            ask(host, |reply| HostRequest::ListWorkspaces { reply }),
        ),
        ("GET", ["api", "workspaces", id, "terminals"]) => reply(
            stream,
            decode(id).and_then(|workspace_id| {
                ask(host, |reply| HostRequest::ListTerminals {
                    workspace_id,
                    reply,
                })
                .flatten()
            }),
        ),
        ("POST", ["api", "workspaces", id, "terminals"]) => reply(
            stream,
            decode(id).and_then(|workspace_id| {
                ask(host, |reply| HostRequest::CreateTerminal {
                    workspace_id,
                    reply,
                })
                .flatten()
            }),
        ),
        ("GET", ["api", "workspaces", id, "tasks"]) => reply(
            stream,
            decode(id).and_then(|workspace_id| {
                ask(host, |reply| HostRequest::ListTasks {
                    workspace_id,
                    reply,
                })
                .flatten()
            }),
        ),
        ("DELETE", ["api", "workspaces", id, "terminals", tab]) => {
            let closed = decode(id).and_then(|workspace_id| {
                let tab_id = tab.parse::<u64>().map_err(|_| NOT_FOUND.to_string())?;
                ask(host, |reply| HostRequest::CloseTerminal {
                    workspace_id,
                    tab_id,
                    reply,
                })
                .flatten()
            });
            reply(stream, closed.map(|()| serde_json::json!({})))
        }
        _ => http::write_json_error(stream, 404, NOT_FOUND),
    }
}

const NOT_FOUND: &str = "not found";

fn decode(segment: &str) -> Result<String, String> {
    percent_decode_str(segment)
        .decode_utf8()
        .ok()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| NOT_FOUND.to_string())
}

fn reply<T: serde::Serialize>(stream: &mut TcpStream, result: Result<T, String>) -> Result<()> {
    match result {
        Ok(value) => http::write_json(stream, &value),
        Err(e) => http::write_json_error(stream, 404, &e),
    }
}

pub(crate) fn bearer(req: &Request) -> &str {
    req.header("authorization")
        .and_then(|a| {
            a.strip_prefix("Bearer ")
                .or_else(|| a.strip_prefix("bearer "))
        })
        .unwrap_or("")
        .trim()
}

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
