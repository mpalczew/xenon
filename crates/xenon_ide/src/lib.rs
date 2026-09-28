//! Xenon as a Claude Code "IDE": a localhost WebSocket MCP server that agents
//! running in the terminal connect to. See PROTOCOL.md.

#[cfg(unix)]
mod codex_protocol;
mod endpoint;
mod lock;
mod protocol;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
#[cfg(unix)]
use std::thread;

use anyhow::Result;
use async_channel::Sender;
use serde_json::json;

use endpoint::Endpoint;

#[cfg(unix)]
use codex_protocol::{
    serve_connection as serve_codex_connection, write_frame as write_codex_frame,
};

/// A request from a connected agent for the UI to act on.
pub enum IdeCommand {
    OpenFile(PathBuf),
}

/// Snapshot of the active editor selection for Claude context.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SelectionSnapshot {
    pub path: PathBuf,
    pub text: String,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// A running IDE server: one endpoint per workspace root, so each agent only
/// sees its own workspace. Dropping it removes the discovery lock files.
pub struct IdeServer {
    endpoints: HashMap<PathBuf, Endpoint>,
    commands: Sender<IdeCommand>,
    /// All workspace roots, for the Codex context server.
    roots: Arc<RwLock<Vec<PathBuf>>>,
    #[cfg(unix)]
    codex: Option<CodexIdeServer>,
}

impl IdeServer {
    /// Start an endpoint per workspace root and the Codex context server.
    pub fn start(roots: Vec<PathBuf>, commands: Sender<IdeCommand>) -> Result<IdeServer> {
        let shared_roots = Arc::new(RwLock::new(roots.clone()));
        #[cfg(unix)]
        let codex = match CodexIdeServer::start(shared_roots.clone()) {
            Ok(server) => Some(server),
            Err(error) => {
                log::warn!("Codex IDE context server unavailable: {error}");
                None
            }
        };
        let mut server = IdeServer {
            endpoints: HashMap::new(),
            commands,
            roots: shared_roots,
            #[cfg(unix)]
            codex,
        };
        server.update_roots(roots)?;
        Ok(server)
    }

    /// Environment for a terminal in `root` so Claude Code finds that
    /// workspace's endpoint.
    pub fn env(&self, root: &Path) -> Vec<(String, String)> {
        let Some(endpoint) = self.endpoints.get(root) else {
            return Vec::new();
        };
        vec![
            (
                "CLAUDE_CODE_SSE_PORT".to_string(),
                endpoint.port().to_string(),
            ),
            ("ENABLE_IDE_INTEGRATION".to_string(), "true".to_string()),
        ]
    }

    /// Start endpoints for new roots and close those for removed roots.
    pub fn update_roots(&mut self, roots: Vec<PathBuf>) -> Result<()> {
        self.endpoints.retain(|root, _| roots.contains(root));
        for root in &roots {
            if !self.endpoints.contains_key(root) {
                let endpoint = Endpoint::start(root.clone(), self.commands.clone())?;
                self.endpoints.insert(root.clone(), endpoint);
            }
        }
        *self.roots.write().expect("IDE roots lock poisoned") = roots;
        Ok(())
    }

    /// Push an editor selection to the agents connected from `root` only.
    pub fn notify_selection(&self, root: &Path, snap: &SelectionSnapshot) {
        #[cfg(unix)]
        if let Some(codex) = &self.codex {
            codex.set_selection(snap.clone());
        }
        let Some(endpoint) = self.endpoints.get(root) else {
            return;
        };
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
        endpoint.broadcast(&msg);
    }
}

#[cfg(unix)]
struct CodexIdeServer {
    socket_path: PathBuf,
    selection: Arc<Mutex<Option<SelectionSnapshot>>>,
    client_id: String,
    follower: bool,
    contexts: Arc<Mutex<HashMap<String, CodexContext>>>,
}

#[cfg(unix)]
#[derive(Clone)]
struct CodexContext {
    workspaces: Vec<PathBuf>,
    selection: Option<SelectionSnapshot>,
    last_active: u64,
}

#[cfg(unix)]
impl CodexIdeServer {
    fn start(roots: Arc<RwLock<Vec<PathBuf>>>) -> Result<Self> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;

        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
            .unwrap_or_else(|| PathBuf::from(".codex"));
        let dir = codex_home.join("ipc");
        fs::create_dir_all(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        let socket_path = dir.join("ipc.sock");
        let client_id = std::env::var("XENON_SLOT")
            .unwrap_or_else(|_| format!("xenon-{}", std::process::id()))
            .to_ascii_lowercase();
        let selection = Arc::new(Mutex::new(None));
        let contexts = Arc::new(Mutex::new(HashMap::new()));
        // A forced exit can leave the Unix socket path behind after its
        // listener is gone. Reclaim only a path that is provably unreachable;
        // a live listener remains untouched and is handled as a follower below.
        let stale_socket = socket_path.exists()
            && matches!(
                std::os::unix::net::UnixStream::connect(&socket_path),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                    )
            );
        if stale_socket {
            match std::fs::remove_file(&socket_path) {
                Ok(()) => log::info!("removed stale Codex IDE socket"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => {
                fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
                let server_roots = roots.clone();
                let server_contexts = contexts.clone();
                let server_selection = selection.clone();
                let server_client_id = client_id.clone();
                thread::Builder::new()
                    .name("xenon-codex-ide".into())
                    .spawn(move || {
                        for stream in listener.incoming().flatten() {
                            let roots = server_roots.clone();
                            let contexts = server_contexts.clone();
                            let selection = server_selection.clone();
                            thread::spawn(move || {
                                if let Err(error) =
                                    serve_codex_connection(stream, &roots, &selection, &contexts)
                                {
                                    log::debug!("Codex IDE connection ended: {error}");
                                }
                            });
                        }
                    })?;
                contexts
                    .lock()
                    .expect("Codex contexts lock poisoned")
                    .insert(
                        server_client_id,
                        CodexContext {
                            workspaces: roots.read().expect("IDE roots lock poisoned").clone(),
                            selection: None,
                            last_active: 0,
                        },
                    );
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => false,
            Err(error) => return Err(error.into()),
        };
        if !listener {
            let mut stream = std::os::unix::net::UnixStream::connect(&socket_path)?;
            write_codex_frame(
                &mut stream,
                &serde_json::json!({
                    "type": "register", "clientId": client_id,
                    "workspaces": roots.read().expect("IDE roots lock poisoned").clone()
                }),
            )?;
            log::info!("Codex IDE client registered with existing server");
        } else {
            log::info!("Codex IDE context server on {}", socket_path.display());
        }
        Ok(Self {
            socket_path,
            selection,
            client_id,
            follower: !listener,
            contexts,
        })
    }

    fn set_selection(&self, selection: SelectionSnapshot) {
        *self
            .selection
            .lock()
            .expect("Codex selection lock poisoned") = Some(selection);
        let current = self
            .selection
            .lock()
            .expect("Codex selection lock poisoned")
            .clone();
        let workspaces = self
            .contexts
            .lock()
            .expect("Codex contexts lock poisoned")
            .get(&self.client_id)
            .map(|context| context.workspaces.clone())
            .unwrap_or_default();
        self.contexts
            .lock()
            .expect("Codex contexts lock poisoned")
            .insert(
                self.client_id.clone(),
                CodexContext {
                    workspaces,
                    selection: current.clone(),
                    last_active: monotonic_activity(),
                },
            );
        if self.follower
            && let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&self.socket_path)
        {
            let _ = write_codex_frame(
                &mut stream,
                &serde_json::json!({
                    "type": "update", "clientId": self.client_id,
                    "selection": current
                }),
            );
        }
    }
}

#[cfg(unix)]
fn monotonic_activity() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(unix)]
impl Drop for CodexIdeServer {
    fn drop(&mut self) {
        if !self.follower {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(path: &str) -> SelectionSnapshot {
        SelectionSnapshot {
            path: PathBuf::from(path),
            text: "picked".into(),
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 6,
        }
    }

    #[test]
    fn selection_reaches_only_its_workspace() {
        let (tx, _rx) = async_channel::unbounded();
        let a = PathBuf::from("/xenon-test/a");
        let b = PathBuf::from("/xenon-test/b");
        // Built by hand: `start` would also join the live Codex IPC socket.
        let mut server = IdeServer {
            endpoints: HashMap::new(),
            commands: tx,
            roots: Arc::new(RwLock::new(Vec::new())),
            #[cfg(unix)]
            codex: None,
        };
        server
            .update_roots(vec![a.clone(), b.clone()])
            .expect("roots");
        let from_a = server.endpoints[&a].subscribe();
        let from_b = server.endpoints[&b].subscribe();

        server.notify_selection(&a, &snapshot("/xenon-test/a/main.rs"));

        assert!(from_a.try_recv().expect("a notified").contains("picked"));
        assert!(from_b.try_recv().is_err());
        assert_ne!(server.env(&a), server.env(&b));
    }
}
