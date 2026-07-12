//! xero as a Claude Code "IDE": a localhost WebSocket MCP server that agents
//! running in xero's terminal connect to, so Claude Code can open files (and
//! more) in xero. See PROTOCOL.md.

mod lock;
mod protocol;

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread;

use anyhow::Result;
use async_channel::Sender;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{Message, accept_hdr};
use uuid::Uuid;

/// A request from a connected agent for the UI to act on.
pub enum IdeCommand {
    OpenFile(PathBuf),
}

/// A running IDE server. Dropping it removes the discovery lock file.
pub struct IdeServer {
    port: u16,
    token: String,
    lock_path: PathBuf,
    roots: Arc<RwLock<Vec<PathBuf>>>,
}

impl IdeServer {
    /// Bind a localhost port, write the lock file, and start accepting agents.
    pub fn start(roots: Vec<PathBuf>, commands: Sender<IdeCommand>) -> Result<IdeServer> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let token = Uuid::new_v4().to_string();
        let lock_path = lock::write(port, &token, &roots)?;
        let roots = Arc::new(RwLock::new(roots));
        log::info!("xero IDE server on 127.0.0.1:{port}");

        let server_roots = roots.clone();
        let server_token = token.clone();
        thread::Builder::new()
            .name("xero-ide".into())
            .spawn(move || accept_loop(listener, server_token, server_roots, commands))?;

        Ok(IdeServer {
            port,
            token,
            lock_path,
            roots,
        })
    }

    /// Environment for the integrated terminal so Claude Code finds this server.
    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            ("CLAUDE_CODE_SSE_PORT".to_string(), self.port.to_string()),
            // Neovim reverse-engineer sets this; VS Code live path mainly used the port.
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
    commands: Sender<IdeCommand>,
) {
    for stream in listener.incoming().flatten() {
        let token = token.clone();
        let roots = roots.clone();
        let commands = commands.clone();
        thread::spawn(move || {
            if let Err(error) = serve_connection(stream, &token, &roots, &commands) {
                log::debug!("xero IDE connection ended: {error}");
            }
        });
    }
}

fn serve_connection(
    stream: std::net::TcpStream,
    token: &str,
    roots: &Arc<RwLock<Vec<PathBuf>>>,
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
    loop {
        match ws.read()? {
            Message::Text(text) => {
                let current_roots = roots.read().expect("IDE roots lock poisoned").clone();
                if let Some(reply) = protocol::handle(&text, &current_roots, commands) {
                    ws.send(Message::Text(reply))?;
                }
            }
            Message::Close(_) => return Ok(()),
            _ => {}
        }
    }
}
