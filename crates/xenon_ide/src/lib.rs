//! Xenon as a Claude Code "IDE": a localhost WebSocket MCP server that agents
//! running in the terminal connect to. See PROTOCOL.md.

mod lock;
mod protocol;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender as MpscSender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use async_channel::Sender;
use serde_json::json;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{Message, accept_hdr};
use uuid::Uuid;

/// A request from a connected agent for the UI to act on.
pub enum IdeCommand {
    OpenFile(PathBuf),
}

/// Snapshot of the active editor selection for Claude context.
#[derive(Clone, Debug)]
pub struct SelectionSnapshot {
    pub path: PathBuf,
    pub text: String,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// A running IDE server. Dropping it removes the discovery lock file.
pub struct IdeServer {
    port: u16,
    token: String,
    lock_path: PathBuf,
    roots: Arc<RwLock<Vec<PathBuf>>>,
    /// Outbound notify queues for connected CLI clients (selection, etc.).
    clients: Arc<Mutex<Vec<MpscSender<String>>>>,
}

impl IdeServer {
    /// Bind a localhost port, write the lock file, and start accepting agents.
    pub fn start(roots: Vec<PathBuf>, commands: Sender<IdeCommand>) -> Result<IdeServer> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let token = Uuid::new_v4().to_string();
        let lock_path = lock::write(port, &token, &roots)?;
        let roots = Arc::new(RwLock::new(roots));
        let clients: Arc<Mutex<Vec<MpscSender<String>>>> = Arc::new(Mutex::new(Vec::new()));
        log::info!("xenon IDE server on 127.0.0.1:{port}");

        let server_roots = roots.clone();
        let server_token = token.clone();
        let server_clients = clients.clone();
        thread::Builder::new()
            .name("xenon-ide".into())
            .spawn(move || {
                accept_loop(
                    listener,
                    server_token,
                    server_roots,
                    server_clients,
                    commands,
                )
            })?;

        Ok(IdeServer {
            port,
            token,
            lock_path,
            roots,
            clients,
        })
    }

    /// Environment for the integrated terminal so Claude Code finds this server.
    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            ("CLAUDE_CODE_SSE_PORT".to_string(), self.port.to_string()),
            ("ENABLE_IDE_INTEGRATION".to_string(), "true".to_string()),
        ]
    }

    /// Replace the advertised workspace folders without changing the server port.
    pub fn update_roots(&mut self, roots: Vec<PathBuf>) -> Result<()> {
        {
            let mut current = self.roots.write().expect("IDE roots lock poisoned");
            *current = roots.clone();
        }
        self.lock_path = lock::write(self.port, &self.token, &roots)?;
        Ok(())
    }

    /// Push current editor selection to connected Claude CLI clients.
    pub fn notify_selection(&self, snap: &SelectionSnapshot) {
        let is_empty = snap.text.is_empty();
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "selection_changed",
            "params": {
                "text": snap.text,
                "filePath": snap.path.to_string_lossy(),
                "fileUrl": format!("file://{}", snap.path.display()),
                "selection": {
                    "start": { "line": snap.start_line, "character": snap.start_character },
                    "end": { "line": snap.end_line, "character": snap.end_character },
                    "isEmpty": is_empty,
                }
            }
        })
        .to_string();
        let mut clients = self.clients.lock().expect("IDE clients lock poisoned");
        clients.retain(|tx| tx.send(msg.clone()).is_ok());
    }
}

impl Drop for IdeServer {
    fn drop(&mut self) {
        lock::remove(&self.lock_path);
    }
}

fn accept_loop(
    listener: TcpListener,
    token: String,
    roots: Arc<RwLock<Vec<PathBuf>>>,
    clients: Arc<Mutex<Vec<MpscSender<String>>>>,
    commands: Sender<IdeCommand>,
) {
    for stream in listener.incoming().flatten() {
        let token = token.clone();
        let roots = roots.clone();
        let clients = clients.clone();
        let commands = commands.clone();
        thread::spawn(move || {
            if let Err(error) = serve_connection(stream, &token, &roots, &clients, &commands) {
                log::debug!("xenon IDE connection ended: {error}");
            }
        });
    }
}

fn serve_connection(
    stream: std::net::TcpStream,
    token: &str,
    roots: &Arc<RwLock<Vec<PathBuf>>>,
    clients: &Arc<Mutex<Vec<MpscSender<String>>>>,
    commands: &Sender<IdeCommand>,
) -> Result<()> {
    let expected = token.to_string();
    #[allow(clippy::result_large_err)]
    let auth =
        move |req: &Request, res: Response| -> std::result::Result<Response, ErrorResponse> {
            let presented = req
                .headers()
                .get("x-claude-code-ide-authorization")
                .and_then(|v| v.to_str().ok());
            if presented == Some(expected.as_str()) {
                Ok(res)
            } else {
                Err(ErrorResponse::new(Some("unauthorized".into())))
            }
        };

    let mut ws = accept_hdr(stream, auth)?;
    let (tx, rx) = mpsc::channel::<String>();
    {
        let mut list = clients.lock().expect("IDE clients lock poisoned");
        list.push(tx);
    }
    // Non-blocking so we can interleave outbound selection notifications.
    ws.get_mut().set_nonblocking(true)?;
    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let current_roots = roots.read().expect("IDE roots lock poisoned").clone();
                if let Some(reply) = protocol::handle(&text, &current_roots, commands) {
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
