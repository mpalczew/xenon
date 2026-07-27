use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use crate::LspError;
use crate::framing::{read_message, write_message};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientState {
    Starting,
    Running,
    Failed,
    Stopped,
}

#[derive(Clone, Debug)]
pub enum ClientEvent {
    Notification { method: String, params: Value },
    StateChanged(ClientState),
}

pub struct Request {
    receiver: mpsc::Receiver<Result<Value, LspError>>,
}

impl Request {
    pub fn recv(self) -> Result<Value, LspError> {
        self.receiver
            .recv()
            .map_err(|_| LspError::Io("language server request channel closed".into()))?
    }

    pub fn recv_timeout(self, timeout: Duration) -> Result<Value, LspError> {
        self.receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => LspError::Timeout,
                mpsc::RecvTimeoutError::Disconnected => {
                    LspError::Io("language server request channel closed".into())
                }
            })?
    }
}

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Result<Value, LspError>>>>>;

pub(crate) struct Client {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    state: Arc<Mutex<ClientState>>,
}

impl Client {
    pub(crate) fn spawn(
        command: &str,
        args: &[String],
        event_tx: mpsc::Sender<ClientEvent>,
    ) -> Result<Self, LspError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::Io("language server stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::Io("language server stdout unavailable".into()))?;
        let stdin = Arc::new(Mutex::new(stdin));
        let pending = Pending::default();
        let state = Arc::new(Mutex::new(ClientState::Starting));
        start_reader(
            BufReader::new(stdout),
            Arc::clone(&stdin),
            Arc::clone(&pending),
            Arc::clone(&state),
            event_tx,
        );
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            stdin,
            pending,
            next_id: AtomicU64::new(1),
            state,
        })
    }

    pub(crate) fn state(&self) -> ClientState {
        self.state
            .lock()
            .map_or(ClientState::Failed, |state| *state)
    }

    pub(crate) fn set_running(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = ClientState::Running;
        }
    }

    pub(crate) fn request(&self, method: &str, params: Value) -> Result<Request, LspError> {
        if matches!(self.state(), ClientState::Failed | ClientState::Stopped) {
            return Err(LspError::NotAvailable);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, receiver) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| LspError::Io("pending request lock poisoned".into()))?
            .insert(id, tx);
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        if let Err(error) = self.write(&message) {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            return Err(error);
        }
        Ok(Request { receiver })
    }

    pub(crate) fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        self.write(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    pub(crate) fn shutdown(&self) {
        if matches!(self.state(), ClientState::Stopped) {
            return;
        }
        if let Ok(request) = self.request("shutdown", Value::Null) {
            let _ = request.recv_timeout(Duration::from_secs(1));
        }
        let _ = self.notify("exit", Value::Null);
        if let Ok(mut child) = self.child.lock()
            && child.try_wait().ok().flatten().is_none()
        {
            let _ = child.kill();
        }
        if let Ok(mut state) = self.state.lock() {
            *state = ClientState::Stopped;
        }
    }

    fn write(&self, message: &Value) -> Result<(), LspError> {
        let body =
            serde_json::to_vec(message).map_err(|error| LspError::Protocol(error.to_string()))?;
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| LspError::Io("language server stdin lock poisoned".into()))?;
        write_message(&mut *stdin, &body)
    }
}

fn start_reader(
    mut reader: impl std::io::BufRead + Send + 'static,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Pending,
    state: Arc<Mutex<ClientState>>,
    event_tx: mpsc::Sender<ClientEvent>,
) {
    thread::spawn(move || {
        while let Ok(Some(body)) = read_message(&mut reader) {
            let Ok(message) = serde_json::from_slice::<Value>(&body) else {
                continue;
            };
            if message.get("method").is_some() && message.get("id").is_some() {
                respond_to_server(&stdin, &message);
            } else if let Some(method) = message.get("method").and_then(Value::as_str) {
                let _ = event_tx.send(ClientEvent::Notification {
                    method: method.to_string(),
                    params: message.get("params").cloned().unwrap_or(Value::Null),
                });
            } else {
                resolve_response(&pending, &message);
            }
        }
        fail_pending(&pending);
        if let Ok(mut current) = state.lock()
            && *current != ClientState::Stopped
        {
            *current = ClientState::Failed;
            let _ = event_tx.send(ClientEvent::StateChanged(ClientState::Failed));
        }
    });
}

fn resolve_response(pending: &Pending, message: &Value) {
    let Some(id) = message.get("id").and_then(Value::as_u64) else {
        return;
    };
    let sender = pending.lock().ok().and_then(|mut map| map.remove(&id));
    let Some(sender) = sender else {
        return;
    };
    let result = match message.get("error") {
        Some(error) => Err(LspError::Rpc(error.to_string())),
        None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
    };
    let _ = sender.send(result);
}

fn respond_to_server(stdin: &Arc<Mutex<ChildStdin>>, message: &Value) {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let response = match method {
        "workspace/configuration" => {
            let count = message
                .pointer("/params/items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            json!({"jsonrpc":"2.0", "id":id, "result":vec![Value::Null; count]})
        }
        "window/workDoneProgress/create"
        | "client/registerCapability"
        | "client/unregisterCapability" => {
            json!({"jsonrpc":"2.0", "id":id, "result":Value::Null})
        }
        "workspace/applyEdit" => {
            json!({"jsonrpc":"2.0", "id":id, "result":{"applied":false}})
        }
        _ => json!({
            "jsonrpc":"2.0",
            "id":id,
            "error":{"code":-32601, "message":"Method not found"}
        }),
    };
    if let Ok(body) = serde_json::to_vec(&response)
        && let Ok(mut writer) = stdin.lock()
    {
        let _ = write_message(&mut *writer, &body);
    }
}

fn fail_pending(pending: &Pending) {
    let senders = pending
        .lock()
        .map(|mut map| map.drain().map(|(_, sender)| sender).collect::<Vec<_>>())
        .unwrap_or_default();
    for sender in senders {
        let _ = sender.send(Err(LspError::Io("language server exited".into())));
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.shutdown();
    }
}
