//! Per-field typing history. Cmd-Z restores the previous text and caret.

use crate::MultilineText;

#[derive(Clone)]
struct Snapshot {
    text: String,
    caret: usize,
    anchor: usize,
}

#[derive(Default)]
pub(super) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl History {
    pub(super) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub(super) fn record(&mut self, before: (String, usize, usize)) {
        self.undo.push(Snapshot {
            text: before.0,
            caret: before.1,
            anchor: before.2,
        });
        self.redo.clear();
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
    }

    pub(super) fn undo(&mut self, value: &mut MultilineText) -> bool {
        self.swap(value, true)
    }

    pub(super) fn redo(&mut self, value: &mut MultilineText) -> bool {
        self.swap(value, false)
    }

    fn swap(&mut self, value: &mut MultilineText, undo: bool) -> bool {
        let (from, to) = if undo {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(previous) = from.pop() else {
            return false;
        };
        let current = value.points();
        to.push(Snapshot {
            text: current.0,
            caret: current.1,
            anchor: current.2,
        });
        value.restore(previous.text, previous.caret, previous.anchor);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_restores_the_previous_text() {
        let mut value = MultilineText::new("ab");
        let mut history = History::default();
        history.record(("a".into(), 1, 1));
        assert!(history.undo(&mut value));
        assert_eq!(value.text(), "a");
        assert!(history.redo(&mut value));
        assert_eq!(value.text(), "ab");
    }
}
