use std::path::Path;

use super::model::TaskSet;
use super::rows::{row_command, with_preview_ctx};
use super::trust::folder_open_closure;

pub fn folder_open_labels(set: &TaskSet) -> Vec<String> {
    set.tasks
        .iter()
        .filter(|t| t.run_on_folder_open && t.unsupported.is_none())
        .map(|t| t.label.clone())
        .collect()
}

pub fn trust_lines(set: &TaskSet, folder: &Path) -> Vec<(String, String, &'static str)> {
    with_preview_ctx(set, folder, |ctx| {
        folder_open_closure(set)
            .into_iter()
            .map(|t| {
                let why = if t.run_on_folder_open {
                    "runs on open"
                } else {
                    "dependency"
                };
                (t.label.clone(), row_command(t, ctx), why)
            })
            .collect()
    })
}

pub fn trust_title(folder: &Path) -> String {
    let name = folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("Run tasks automatically in {name}?")
}
