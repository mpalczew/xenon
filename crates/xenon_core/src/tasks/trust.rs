//! folderOpen tasks and the hash that keys their approval.

use sha2::{Digest, Sha256};

use super::model::{TaskDef, TaskSet};

/// folderOpen tasks plus everything they depend on, each once, in file order.
/// This is exactly what an approval covers.
pub fn folder_open_closure(set: &TaskSet) -> Vec<&TaskDef> {
    let mut wanted: Vec<&str> = set
        .tasks
        .iter()
        .filter(|t| t.run_on_folder_open && t.unsupported.is_none())
        .map(|t| t.label.as_str())
        .collect();
    let mut i = 0;
    while i < wanted.len() {
        if let Some(def) = set.get(wanted[i]) {
            for dep in &def.depends_on {
                if !wanted.contains(&dep.as_str()) {
                    wanted.push(dep);
                }
            }
        }
        i += 1;
    }
    set.tasks
        .iter()
        .filter(|t| wanted.contains(&t.label.as_str()))
        .collect()
}

/// Hex SHA-256 of the merged definitions in the closure (every field that
/// affects what runs, variables unexpanded). `None` when no task runs on open.
pub fn folder_open_hash(set: &TaskSet) -> Option<String> {
    let closure = folder_open_closure(set);
    if !closure.iter().any(|t| t.run_on_folder_open) {
        return None;
    }
    let json = serde_json::to_string(&closure).ok()?;
    let digest = Sha256::digest(json.as_bytes());
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}
