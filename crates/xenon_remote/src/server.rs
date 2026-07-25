//! Local HTTP-only server for mobile PTY remote (lean stack).

use std::collections::HashMap;
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use uuid::Uuid;

use crate::auth::token_ok;
use crate::host::{HostRequest, HostTx, ViewportSnapshot};
use crate::http;
use crate::png_frame::viewport_png;
use crate::protocol::{AuthRequest, AuthResponse, InjectRequest, TerminalInfo, WorkspaceInfo};

const PAGE: &str = include_str!("page.html");
const TICKET_TTL_SECS: u64 = 60 * 60 * 12;

struct TicketStore {
    map: Mutex<HashMap<String, u64>>,
}

impl TicketStore {
    fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    fn issue(&self) -> String {
        let t = Uuid::new_v4().to_string();
        let exp = now_secs() + TICKET_TTL_SECS;
        self.map.lock().expect("ticket lock").insert(t.clone(), exp);
        t
    }

    fn valid(&self, ticket: &str) -> bool {
        let mut map = self.map.lock().expect("ticket lock");
        let now = now_secs();
        map.retain(|_, exp| *exp > now);
        map.contains_key(ticket)
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Running mobile remote server. Drop stops accepting.
pub struct RemoteServer {
    pub port: u16,
    pub token: String,
    pub bind: SocketAddr,
    enabled: Arc<AtomicBool>,
    _join: Option<thread::JoinHandle<()>>,
}

impl RemoteServer {
    /// Bind `0.0.0.0:port` (port 0 = ephemeral). Host fulfills list/frame/inject.
    /// `token` is the shared password; empty is rejected.
    pub fn start(port: u16, token: String, host: HostTx) -> Result<Self> {
        if token.is_empty() {
            anyhow::bail!("remote password must not be empty");
        }
        let listener = TcpListener::bind(("0.0.0.0", port))
            .with_context(|| format!("bind remote on 0.0.0.0:{port}"))?;
        listener.set_nonblocking(false)?;
        let addr = listener.local_addr()?;
        let enabled = Arc::new(AtomicBool::new(true));
        let tickets = Arc::new(TicketStore::new());
        let token_arc = Arc::new(token.clone());
        let en = enabled.clone();
        let join = thread::Builder::new()
            .name("xenon-remote".into())
            .spawn(move || accept_loop(listener, en, token_arc, tickets, host))?;
        log::info!("xenon remote listening on http://{addr}/");
        Ok(Self {
            port: addr.port(),
            token,
            bind: addr,
            enabled,
            _join: Some(join),
        })
    }

    pub fn stop(&self) {
        self.enabled.store(false, Ordering::SeqCst);
        let _ = TcpStream::connect(self.bind);
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn accept_loop(
    listener: TcpListener,
    enabled: Arc<AtomicBool>,
    token: Arc<String>,
    tickets: Arc<TicketStore>,
    host: HostTx,
) {
    for stream in listener.incoming() {
        if !enabled.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let token = token.clone();
        let tickets = tickets.clone();
        let host = host.clone();
        let enabled = enabled.clone();
        thread::spawn(move || {
            if !enabled.load(Ordering::SeqCst) {
                return;
            }
            if let Err(e) = handle_connection(stream, &token, &tickets, &host) {
                log::debug!("xenon remote connection: {e:#}");
            }
        });
    }
}

#[allow(clippy::too_many_lines)]
fn handle_connection(
    mut stream: TcpStream,
    token: &str,
    tickets: &TicketStore,
    host: &HostTx,
) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut buf = vec![0u8; 16 * 1024];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buf[..n]);
    let (req_line, headers, body_start) = http::split_http(&head)?;
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path_q = parts.next().unwrap_or("/");
    let (path, query) = http::split_path_query(path_q);

    let body = http::extract_body(&buf[..n], body_start, &headers, &mut stream)?;

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            http::write_http(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
            )?;
        }
        ("POST", "/auth") => {
            let req: AuthRequest = serde_json::from_slice(&body).unwrap_or(AuthRequest {
                token: String::new(),
            });
            if token_ok(token, &req.token) {
                let ticket = tickets.issue();
                let resp = AuthResponse { ticket };
                let bytes = serde_json::to_vec(&resp)?;
                http::write_http(&mut stream, 200, "application/json", &bytes)?;
            } else {
                http::write_http(
                    &mut stream,
                    401,
                    "application/json",
                    br#"{"error":"unauthorized"}"#,
                )?;
            }
        }
        ("GET", "/api/workspaces") => {
            if !authorize_bearer(&headers, tickets) {
                http::write_http(
                    &mut stream,
                    401,
                    "application/json",
                    br#"{"error":"unauthorized"}"#,
                )?;
                return Ok(());
            }
            let list = ask_list_workspaces(host)?;
            let bytes = serde_json::to_vec(&list)?;
            http::write_http(&mut stream, 200, "application/json", &bytes)?;
        }
        (m, p) if m == "GET" && p.starts_with("/api/workspaces/") && p.ends_with("/terminals") => {
            if !authorize_bearer(&headers, tickets) {
                http::write_http(
                    &mut stream,
                    401,
                    "application/json",
                    br#"{"error":"unauthorized"}"#,
                )?;
                return Ok(());
            }
            let rest = p
                .strip_prefix("/api/workspaces/")
                .and_then(|s| s.strip_suffix("/terminals"))
                .unwrap_or("");
            let id = http::urlencoding_decode(rest);
            match ask_list_terminals(host, &id) {
                Ok(list) => {
                    let bytes = serde_json::to_vec(&list)?;
                    http::write_http(&mut stream, 200, "application/json", &bytes)?;
                }
                Err(e) => {
                    let body = serde_json::json!({"error": e}).to_string();
                    http::write_http(&mut stream, 404, "application/json", body.as_bytes())?;
                }
            }
        }
        ("GET", "/api/frame") => {
            if !authorize_bearer(&headers, tickets) {
                http::write_http(
                    &mut stream,
                    401,
                    "application/json",
                    br#"{"error":"unauthorized"}"#,
                )?;
                return Ok(());
            }
            let ws_id = http::query_param(query, "workspaceId").unwrap_or("");
            let tab_s = http::query_param(query, "tabId").unwrap_or("0");
            let since: u64 = http::query_param(query, "since")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let tab_id: u64 = match tab_s.parse() {
                Ok(t) => t,
                Err(_) => {
                    http::write_http(
                        &mut stream,
                        400,
                        "application/json",
                        br#"{"error":"invalid tabId"}"#,
                    )?;
                    return Ok(());
                }
            };
            if ws_id.is_empty() {
                http::write_http(
                    &mut stream,
                    400,
                    "application/json",
                    br#"{"error":"workspaceId required"}"#,
                )?;
                return Ok(());
            }
            match ask_frame(host, ws_id, tab_id) {
                Ok(snap) => {
                    if snap.seq == since && since != 0 {
                        http::write_http_status(&mut stream, 204, "No Content", &[])?;
                        return Ok(());
                    }
                    let png = viewport_png(&snap.lines, snap.cols, snap.rows)?;
                    http::write_png_frame(&mut stream, &snap, &png)?;
                }
                Err(e) => {
                    let body = serde_json::json!({"error": e}).to_string();
                    http::write_http(&mut stream, 404, "application/json", body.as_bytes())?;
                }
            }
        }
        ("POST", "/api/inject") => {
            if !authorize_bearer(&headers, tickets) {
                http::write_http(
                    &mut stream,
                    401,
                    "application/json",
                    br#"{"error":"unauthorized"}"#,
                )?;
                return Ok(());
            }
            let req: InjectRequest = match serde_json::from_slice(&body) {
                Ok(r) => r,
                Err(e) => {
                    let body = serde_json::json!({"error": format!("bad body: {e}")}).to_string();
                    http::write_http(&mut stream, 400, "application/json", body.as_bytes())?;
                    return Ok(());
                }
            };
            match ask_inject(host, &req.workspace_id, req.tab_id, req.text) {
                Ok(()) => {
                    http::write_http(&mut stream, 200, "application/json", br#"{"ok":true}"#)?;
                }
                Err(e) => {
                    let body = serde_json::json!({"error": e}).to_string();
                    http::write_http(&mut stream, 400, "application/json", body.as_bytes())?;
                }
            }
        }
        _ => {
            http::write_http(&mut stream, 404, "text/plain", b"not found")?;
        }
    }
    Ok(())
}

fn authorize_bearer(headers: &HashMap<String, String>, tickets: &TicketStore) -> bool {
    let Some(auth) = headers.get("authorization") else {
        return false;
    };
    let ticket = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))
        .unwrap_or("")
        .trim();
    tickets.valid(ticket)
}

fn ask_list_workspaces(host: &HostTx) -> Result<Vec<WorkspaceInfo>> {
    let (tx, rx) = mpsc::sync_channel(1);
    host.send_blocking(HostRequest::ListWorkspaces { reply: tx })
        .map_err(|_| anyhow!("host gone"))?;
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| anyhow!("host timeout"))
}

fn ask_list_terminals(host: &HostTx, workspace_id: &str) -> Result<Vec<TerminalInfo>, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    host.send_blocking(HostRequest::ListTerminals {
        workspace_id: workspace_id.to_string(),
        reply: tx,
    })
    .map_err(|_| "host gone".to_string())?;
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| "host timeout".to_string())?
}

fn ask_frame(host: &HostTx, workspace_id: &str, tab_id: u64) -> Result<ViewportSnapshot, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    host.send_blocking(HostRequest::CaptureFrame {
        workspace_id: workspace_id.to_string(),
        tab_id,
        reply: tx,
    })
    .map_err(|_| "host gone".to_string())?;
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| "host timeout".to_string())?
}

fn ask_inject(host: &HostTx, workspace_id: &str, tab_id: u64, text: String) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel(1);
    host.send_blocking(HostRequest::Inject {
        workspace_id: workspace_id.to_string(),
        tab_id,
        text,
        reply: tx,
    })
    .map_err(|_| "host gone".to_string())?;
    rx.recv_timeout(Duration::from_secs(5))
        .map_err(|_| "host timeout".to_string())?
}

/// Shared monotonic frame sequence (host may also track per-tab).
pub fn next_global_seq() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod http_tests;
