//! One Claude Code IDE endpoint: a localhost WebSocket port plus its discovery
//! lock file, scoped to a single workspace root. Each workspace gets its own
//! endpoint so an agent only sees its own workspace's selection.

use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender as MpscSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use async_channel::Sender;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{Message, accept_hdr};
use uuid::Uuid;

use crate::{IdeCommand, lock, protocol};

type Clients = Arc<Mutex<Vec<MpscSender<String>>>>;

pub(crate) struct Endpoint {
    port: u16,
    lock_path: PathBuf,
    clients: Clients,
    closed: Arc<AtomicBool>,
}

/// State shared by the accept thread and every connection it serves.
#[derive(Clone)]
struct Shared {
    token: String,
    root: PathBuf,
    clients: Clients,
    commands: Sender<IdeCommand>,
    closed: Arc<AtomicBool>,
}

impl Endpoint {
    /// Bind a localhost port for `root`, write its lock file, and accept agents.
    pub(crate) fn start(root: PathBuf, commands: Sender<IdeCommand>) -> Result<Endpoint> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let token = Uuid::new_v4().to_string();
        let lock_path = lock::write(port, &token, std::slice::from_ref(&root))?;
        let shared = Shared {
            token,
            root,
            clients: Arc::new(Mutex::new(Vec::new())),
            commands,
            closed: Arc::new(AtomicBool::new(false)),
        };
        log::info!(
            "xenon IDE endpoint for {} on 127.0.0.1:{port}",
            shared.root.display()
        );
        let endpoint = Endpoint {
            port,
            lock_path,
            clients: shared.clients.clone(),
            closed: shared.closed.clone(),
        };
        thread::Builder::new()
            .name("xenon-ide".into())
            .spawn(move || accept_loop(listener, shared))?;
        Ok(endpoint)
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Register an outbound queue as if an agent had connected.
    #[cfg(test)]
    pub(crate) fn subscribe(&self) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel();
        self.clients
            .lock()
            .expect("IDE clients lock poisoned")
            .push(tx);
        rx
    }

    /// Queue `msg` for every agent connected to this endpoint.
    pub(crate) fn broadcast(&self, msg: &str) {
        let mut clients = self.clients.lock().expect("IDE clients lock poisoned");
        clients.retain(|tx| tx.send(msg.to_string()).is_ok());
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        lock::remove(&self.lock_path);
        // Wake the blocking accept so its thread observes `closed` and exits.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn accept_loop(listener: TcpListener, shared: Shared) {
    for stream in listener.incoming().flatten() {
        if shared.closed.load(Ordering::SeqCst) {
            return;
        }
        let shared = shared.clone();
        thread::spawn(move || {
            if let Err(error) = serve_connection(stream, &shared) {
                log::debug!("xenon IDE connection ended: {error}");
            }
        });
    }
}

/// Claude Code (>= 2.1.283) requests the `mcp` subprotocol and drops the
/// socket unless the handshake echoes it back.
fn with_mcp_subprotocol(req: &Request, mut res: Response) -> Response {
    let requested = req
        .headers()
        .get_all("sec-websocket-protocol")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|p| p.trim() == "mcp");
    if requested {
        res.headers_mut().insert(
            "sec-websocket-protocol",
            tungstenite::http::HeaderValue::from_static("mcp"),
        );
    }
    res
}

fn serve_connection(stream: TcpStream, shared: &Shared) -> Result<()> {
    let expected = shared.token.clone();
    #[allow(clippy::result_large_err)]
    let auth =
        move |req: &Request, res: Response| -> std::result::Result<Response, ErrorResponse> {
            let presented = req
                .headers()
                .get("x-claude-code-ide-authorization")
                .and_then(|v| v.to_str().ok());
            if presented == Some(expected.as_str()) {
                Ok(with_mcp_subprotocol(req, res))
            } else {
                Err(ErrorResponse::new(Some("unauthorized".into())))
            }
        };

    let mut ws = accept_hdr(stream, auth)?;
    let (tx, rx) = mpsc::channel::<String>();
    shared
        .clients
        .lock()
        .expect("IDE clients lock poisoned")
        .push(tx);
    let roots = [shared.root.clone()];
    // Non-blocking so we can interleave outbound selection notifications.
    ws.get_mut().set_nonblocking(true)?;
    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                if let Some(reply) = protocol::handle(&text, &roots, &shared.commands) {
                    ws.get_mut().set_nonblocking(false)?;
                    ws.send(Message::Text(reply))?;
                    ws.get_mut().set_nonblocking(true)?;
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if shared.closed.load(Ordering::SeqCst) {
                    return Ok(());
                }
                while let Ok(msg) = rx.try_recv() {
                    ws.get_mut().set_nonblocking(false)?;
                    ws.send(Message::Text(msg))?;
                    ws.get_mut().set_nonblocking(true)?;
                }
                thread::sleep(Duration::from_millis(40));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(protocol: Option<&str>) -> Request {
        let mut builder = Request::builder().uri("ws://127.0.0.1/");
        if let Some(protocol) = protocol {
            builder = builder.header("sec-websocket-protocol", protocol);
        }
        builder.body(()).unwrap()
    }

    fn echoed(protocol: Option<&str>) -> Option<String> {
        with_mcp_subprotocol(&request(protocol), Response::new(()))
            .headers()
            .get("sec-websocket-protocol")
            .map(|v| v.to_str().unwrap().to_string())
    }

    #[test]
    fn echoes_requested_mcp_subprotocol() {
        assert_eq!(echoed(Some("mcp")).as_deref(), Some("mcp"));
        assert_eq!(echoed(Some("other, mcp")).as_deref(), Some("mcp"));
    }

    #[test]
    fn omits_subprotocol_when_not_requested() {
        assert_eq!(echoed(None), None);
        assert_eq!(echoed(Some("other")), None);
    }
}
