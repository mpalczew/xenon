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
}

#[derive(Default)]
pub struct UndoStack {
    past: Vec<Transaction>,
    future: Vec<Transaction>,
    /// When set, new edits join this open group instead of starting a new step.
    open: Option<Vec<Edit>>,
}

impl UndoStack {
    /// Start grouping edits into one undo step. Nested calls are no-ops.
    pub fn begin_group(&mut self) {
        if self.open.is_none() {
            self.open = Some(Vec::new());
        }
    }

    /// Close the open group and push it as one undo step (if non-empty).
    pub fn end_group(&mut self) {
        if let Some(edits) = self.open.take()
            && !edits.is_empty()
        {
            self.past.push(Transaction { edits });
            self.future.clear();
        }
    }

    pub fn push(&mut self, edit: Edit) {
        if edit.old.is_empty() && edit.new.is_empty() {
            return;
        }
        if let Some(group) = &mut self.open {
            group.push(edit);
            return;
        }
        self.past.push(Transaction { edits: vec![edit] });
        self.future.clear();
    }

    /// Pop the last step. Returns edits in reverse apply order for undoing.
    pub fn undo(&mut self) -> Option<Vec<Edit>> {
        // Finalize an unfinished group so Esc was not required to record it.
        self.end_group();
        let tx = self.past.pop()?;
        let mut edits = tx.edits.clone();
        self.future.push(tx);
        edits.reverse();
        Some(edits)
    }

    /// Re-apply the last undone step. Returns edits in forward apply order.
    pub fn redo(&mut self) -> Option<Vec<Edit>> {
        let tx = self.future.pop()?;
        let edits = tx.edits.clone();
        self.past.push(tx);
        Some(edits)
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
        stack.push(edit.clone());
        assert_eq!(stack.undo(), Some(vec![edit.clone()]));
        assert_eq!(stack.redo(), Some(vec![edit]));
        assert!(stack.redo().is_none());
    }

    #[test]
    fn group_is_one_undo_step() {
        let mut stack = UndoStack::default();
        stack.begin_group();
        stack.push(Edit {
            start: 0,
            old: String::new(),
            new: "a".into(),
        });
        stack.push(Edit {
            start: 1,
            old: String::new(),
            new: "b".into(),
        });
        stack.end_group();
        let undone = stack.undo().unwrap();
        // Reverse order for apply: undo b then a.
        assert_eq!(undone.len(), 2);
        assert_eq!(undone[0].new, "b");
        assert_eq!(undone[1].new, "a");
        assert!(stack.undo().is_none());
    }

    #[test]
    fn ungrouped_edits_are_separate_steps() {
        let mut stack = UndoStack::default();
        stack.push(Edit {
            start: 0,
            old: String::new(),
            new: "a".into(),
        });
        stack.push(Edit {
            start: 1,
            old: String::new(),
            new: "b".into(),
        });
        assert_eq!(stack.undo().unwrap().len(), 1);
        assert_eq!(stack.undo().unwrap()[0].new, "a");
    }
}
