//! A JSON picture of what the model says is on screen, published as the state
//! of the `app.ui-state` action so scripts can check layout, tabs, and status
//! over D-Bus without parsing pixels.

use serde_json::{Value, json};
use xenon_core::WorkspaceDot;

use crate::model::Model;

fn dot_name(dot: Option<WorkspaceDot>) -> Value {
    match dot {
        Some(WorkspaceDot::Working) => json!("working"),
        Some(WorkspaceDot::Attention(_)) => json!("attention"),
        None => Value::Null,
    }
}

impl Model {
    pub(crate) fn describe(&self) -> String {
        let active = self.active();
        let workspaces: Vec<Value> = self
            .registry
            .workspaces
            .iter()
            .map(|rec| {
                json!({
                    "name": rec.name,
                    "root": rec.root,
                    "active": Some(rec.id) == active,
                    "dot": dot_name(self.workspace_dot(rec.id)),
                })
            })
            .collect();
        let mut panes = Vec::new();
        let mut focused = Value::Null;
        if let Some(space) = active.and_then(|id| self.spaces.get(&id)) {
            focused = json!(space.content.focused.map(|p| p.0));
            if let Some(root) = space.content.root.as_ref() {
                let ws = active.unwrap_or_default();
                root.for_each_leaf(&mut |leaf| {
                    let tabs: Vec<Value> = leaf
                        .tabs
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            json!({
                                "id": t.id.0,
                                "label": t.label(i),
                                "dot": dot_name(self.tab_dot(ws, t.id)),
                                "exited": t.exited,
                            })
                        })
                        .collect();
                    panes.push(json!({"id": leaf.id.0, "active": leaf.active, "tabs": tabs}));
                });
            }
        }
        json!({"workspaces": workspaces, "panes": panes, "focused_pane": focused}).to_string()
    }
}
