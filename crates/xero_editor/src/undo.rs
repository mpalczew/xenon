//! Minimal edit history for undo/redo.

/// One replace of `old` with `new` at `start` (char indices).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub old: String,
    pub new: String,
}

#[derive(Default)]
pub struct UndoStack {
    past: Vec<Edit>,
    future: Vec<Edit>,
}

impl UndoStack {
    pub fn push(&mut self, edit: Edit) {
        if edit.old.is_empty() && edit.new.is_empty() {
            return;
        }
        self.past.push(edit);
        self.future.clear();
    }

    pub fn undo(&mut self) -> Option<Edit> {
        let edit = self.past.pop()?;
        self.future.push(edit.clone());
        Some(edit)
    }

    pub fn redo(&mut self) -> Option<Edit> {
        let edit = self.future.pop()?;
        self.past.push(edit.clone());
        Some(edit)
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
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
        assert_eq!(stack.undo(), Some(edit.clone()));
        assert_eq!(stack.redo(), Some(edit));
        assert!(stack.redo().is_none());
    }
}
