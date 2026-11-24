use std::collections::HashMap;

use xenon_core::WorkspaceId;
use xenon_worklist::{Effect, ItemText, Op, Snapshot, Undo, apply, read, revert};

use crate::remote::{WorklistEdit, WorklistGroup, WorklistInfo, WorklistItem};
use crate::{Phone, locked};

const GONE: &str = "That changed on your computer.";
const NO_UNDO: &str = "Can\u{2019}t undo \u{2014} the worklist changed since.";

#[derive(Default)]
pub(crate) struct Undos {
    next: u64,
    last: HashMap<WorkspaceId, (u64, Undo)>,
}

fn info(now: Snapshot) -> WorklistInfo {
    let groups = now
        .groups
        .into_iter()
        .map(|group| WorklistGroup {
            place: group.place.to_string(),
            label: group.label,
            items: group
                .items
                .into_iter()
                .map(|item| WorklistItem {
                    key: item.key.to_string(),
                    title: item.title,
                    details: item.details,
                    done: item.done,
                    editable: item.editable,
                })
                .collect(),
        })
        .collect();
    WorklistInfo {
        groups,
        notice: None,
        undo: None,
    }
}

fn bad<E>(_: E) -> String {
    "not found".into()
}

impl Phone {
    pub(crate) fn worklist(&self, workspace_id: &str) -> Result<WorklistInfo, String> {
        let (_, root) = Self::workspace_root(workspace_id)?;
        read(&root).map(info).map_err(|e| e.to_string())
    }

    pub(crate) fn edit_worklist(
        &self,
        workspace_id: &str,
        edit: WorklistEdit,
    ) -> Result<WorklistInfo, String> {
        let (ws, root) = Self::workspace_root(workspace_id)?;
        let op = match edit {
            WorklistEdit::SetDone { item, done } => Op::SetDone {
                item: item.parse().map_err(bad)?,
                done,
            },
            WorklistEdit::Add { place, title } => Op::Add {
                to: place.parse().map_err(bad)?,
                text: ItemText::task(&title).map_err(|e| e.to_string())?,
            },
            WorklistEdit::Remove { item } => Op::Remove {
                item: item.parse().map_err(bad)?,
            },
            WorklistEdit::Undo { token } => {
                let held = {
                    let mut undos = locked(&self.worklist_undo);
                    undos.last.remove(&ws).filter(|(held, _)| *held == token)
                };
                let reverted = held.is_some_and(|(_, undo)| revert(&root, &undo).is_ok());
                let mut fresh = self.worklist(workspace_id)?;
                fresh.notice = (!reverted).then(|| NO_UNDO.to_string());
                return Ok(fresh);
            }
        };
        let removing = matches!(op, Op::Remove { .. });
        let applied = apply(&root, op).map_err(|e| e.to_string())?;
        let mut reply = info(applied.now);
        match applied.effect {
            Effect::Gone => reply.notice = Some(GONE.into()),
            Effect::Saved { undo, .. } if removing => {
                let mut undos = locked(&self.worklist_undo);
                undos.next += 1;
                let token = undos.next;
                undos.last.insert(ws, (token, undo));
                reply.undo = Some(token);
            }
            _ => {}
        }
        Ok(reply)
    }
}
