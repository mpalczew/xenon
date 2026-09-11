use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use anyhow::Result;

use super::{CodexContext, SelectionSnapshot};

pub(super) fn serve_connection(
    mut stream: std::os::unix::net::UnixStream,
    roots: &Arc<RwLock<Vec<PathBuf>>>,
    selection: &Arc<Mutex<Option<SelectionSnapshot>>>,
    contexts: &Arc<Mutex<HashMap<String, CodexContext>>>,
) -> Result<()> {
    let request = read_request(&mut stream)?;
    if matches!(
        request.get("type").and_then(serde_json::Value::as_str),
        Some("register") | Some("update")
    ) {
        update_context(&request, contexts);
        return Ok(());
    }
    let Some(request_id) = request.get("requestId").and_then(serde_json::Value::as_str) else {
        return Ok(());
    };
    if request.get("method").and_then(serde_json::Value::as_str) != Some("ide-context") {
        return Ok(());
    }
    let workspace = request
        .pointer("/params/workspaceRoot")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !workspace_is_open(roots, workspace) {
        write_frame(
            &mut stream,
            &serde_json::json!({
                "type": "response",
                "requestId": request_id,
                "resultType": "error",
                "method": "ide-context",
                "error": "workspace is not open in Xenon"
            }),
        )?;
        return Ok(());
    }
    write_frame(
        &mut stream,
        &context_response(request_id, workspace, selection, contexts),
    )?;
    Ok(())
}

fn read_request(stream: &mut std::os::unix::net::UnixStream) -> Result<serde_json::Value> {
    use std::io::Read;

    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let size = u32::from_le_bytes(length) as usize;
    anyhow::ensure!(size <= 256 * 1024 * 1024, "Codex IDE request is too large");
    let mut payload = vec![0; size];
    stream.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}

fn update_context(
    request: &serde_json::Value,
    contexts: &Arc<Mutex<HashMap<String, CodexContext>>>,
) {
    let Some(client_id) = request.get("clientId").and_then(serde_json::Value::as_str) else {
        return;
    };
    if client_id.is_empty() {
        return;
    }
    let mut all = contexts.lock().expect("Codex contexts lock poisoned");
    let entry = all.entry(client_id.to_string()).or_insert(CodexContext {
        workspaces: Vec::new(),
        selection: None,
        last_active: 0,
    });
    if let Some(workspaces) = request
        .get("workspaces")
        .and_then(serde_json::Value::as_array)
    {
        entry.workspaces = workspaces
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(PathBuf::from)
            .collect();
    }
    if let Some(selection) = request.get("selection") {
        entry.selection = serde_json::from_value(selection.clone()).ok();
        entry.last_active = super::monotonic_activity();
    }
}

fn workspace_is_open(roots: &Arc<RwLock<Vec<PathBuf>>>, workspace: &str) -> bool {
    roots
        .read()
        .expect("IDE roots lock poisoned")
        .iter()
        .any(|root| root.to_string_lossy() == workspace)
}

fn context_response(
    request_id: &str,
    workspace: &str,
    selection: &Arc<Mutex<Option<SelectionSnapshot>>>,
    contexts: &Arc<Mutex<HashMap<String, CodexContext>>>,
) -> serde_json::Value {
    let active = contexts
        .lock()
        .expect("Codex contexts lock poisoned")
        .values()
        .filter(|context| {
            context
                .workspaces
                .iter()
                .any(|root| root.to_string_lossy() == workspace)
        })
        .max_by_key(|context| context.last_active)
        .and_then(|context| context.selection.clone())
        .or_else(|| {
            selection
                .lock()
                .expect("Codex selection lock poisoned")
                .clone()
        })
        .map(|snap| {
            serde_json::json!({
                "label": snap.path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
                "path": snap.path.strip_prefix(workspace).unwrap_or(&snap.path),
                "fsPath": snap.path,
                "selection": {
                    "start": { "line": snap.start_line, "character": snap.start_character },
                    "end": { "line": snap.end_line, "character": snap.end_character }
                },
                "activeSelectionContent": snap.text,
                "selections": []
            })
        });
    let context = serde_json::json!({"activeFile": active, "openTabs": []});
    serde_json::json!({
        "type": "response",
        "requestId": request_id,
        "resultType": "success",
        "method": "ide-context",
        "handledByClientId": "xenon-client",
        "result": { "type": "broadcast", "ideContext": context }
    })
}

pub(super) fn write_frame(
    stream: &mut std::os::unix::net::UnixStream,
    message: &serde_json::Value,
) -> Result<()> {
    use std::io::Write;

    let bytes = serde_json::to_vec(message)?;
    anyhow::ensure!(
        u32::try_from(bytes.len()).is_ok(),
        "Codex IDE response is too large"
    );
    stream.write_all(&(bytes.len() as u32).to_le_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}
