//! Edit history for undo/redo.
//!
//! Edits can be grouped into a single undo step (vim insert session = one `u`).
//! Matches classic vim / Zed: everything from entering insert until Esc is one
//! change; each normal-mode command is its own step. Repeated `u` walks further.

/// One replace of `old` with `new` at `start` (char indices).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub old: String,
    pub new: String,
}

/// A single undoable unit: one or more atomic edits applied in order.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Transaction {
    edits: Vec<Edit>,
    /// Buffer content version before this transaction was applied.
    version_before: u64,
}

#[derive(Default)]
pub struct UndoStack {
    past: Vec<Transaction>,
    future: Vec<Transaction>,
    /// When set, new edits join this open group instead of starting a new step.
    open: Option<OpenGroup>,
}

#[derive(Default)]
struct OpenGroup {
    edits: Vec<Edit>,
    /// Set on the first edit of the group.
    version_before: Option<u64>,
}

impl UndoStack {
    /// Start grouping edits into one undo step. Nested calls are no-ops.
    pub fn begin_group(&mut self) {
        if self.open.is_none() {
            self.open = Some(OpenGroup::default());
        }
    }

    /// Close the open group and push it as one undo step (if non-empty).
    pub fn end_group(&mut self) {
        if let Some(group) = self.open.take()
            && !group.edits.is_empty()
        {
            self.past.push(Transaction {
                edits: group.edits,
                version_before: group.version_before.unwrap_or(0),
            });
            self.future.clear();
        }
    }

    /// Whether the open group already has at least one edit (and thus a version).
    pub fn group_has_edits(&self) -> bool {
        self.open.as_ref().is_some_and(|g| !g.edits.is_empty())
    }

    pub fn push(&mut self, edit: Edit, version_before: u64) {
        if edit.old.is_empty() && edit.new.is_empty() {
            return;
        }
        if let Some(group) = &mut self.open {
            if group.edits.is_empty() {
                group.version_before = Some(version_before);
            }
            group.edits.push(edit);
            return;
        }
        self.past.push(Transaction {
            edits: vec![edit],
            version_before,
        });
        self.future.clear();
    }

    /// Pop the last step. Returns edits (reverse apply order) and version to restore.
    pub fn undo(&mut self) -> Option<(Vec<Edit>, u64)> {
        // Finalize an unfinished group so Esc was not required to record it.
        self.end_group();
        let tx = self.past.pop()?;
        let version_before = tx.version_before;
        let mut edits = tx.edits.clone();
        self.future.push(tx);
        edits.reverse();
        Some((edits, version_before))
    }

    /// Re-apply the last undone step. Returns edits and the version after re-apply
    /// (`version_before + 1`).
    pub fn redo(&mut self) -> Option<(Vec<Edit>, u64)> {
        let tx = self.future.pop()?;
        let version_after = tx.version_before.saturating_add(1);
        let edits = tx.edits.clone();
        self.past.push(tx);
        Some((edits, version_after))
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.open = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_round_trip() {
        let mut stack = UndoStack::default();
        let edit = Edit {
            start: 0,
            old: String::new(),
            new: "hi".into(),
        };
        stack.push(edit.clone(), 0);
        assert_eq!(stack.undo(), Some((vec![edit.clone()], 0)));
        assert_eq!(stack.redo(), Some((vec![edit], 1)));
        assert!(stack.redo().is_none());
    }

    #[test]
    fn group_is_one_undo_step() {
        let mut stack = UndoStack::default();
        stack.begin_group();
        stack.push(
            Edit {
                start: 0,
                old: String::new(),
                new: "a".into(),
            },
            0,
        );
        stack.push(
            Edit {
                start: 1,
                old: String::new(),
                new: "b".into(),
            },
            0, // ignored; only first edit's version_before sticks
        );
        stack.end_group();
        let (undone, version_before) = stack.undo().unwrap();
        // Reverse order for apply: undo b then a.
        assert_eq!(undone.len(), 2);
        assert_eq!(undone[0].new, "b");
        assert_eq!(undone[1].new, "a");
        assert_eq!(version_before, 0);
        assert!(stack.undo().is_none());
    }

    #[test]
    fn ungrouped_edits_are_separate_steps() {
        let mut stack = UndoStack::default();
        stack.push(
            Edit {
                start: 0,
                old: String::new(),
                new: "a".into(),
            },
            0,
        );
        stack.push(
            Edit {
                start: 1,
                old: String::new(),
                new: "b".into(),
            },
            1,
        );
        let (edits, v) = stack.undo().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(v, 1);
        let (edits, v) = stack.undo().unwrap();
        assert_eq!(edits[0].new, "a");
        assert_eq!(v, 0);
    }
}
