//! What the service tells everyone who listens: windows and the phone server.

use std::sync::{Arc, Mutex};

use xenon_core::{TermId, WorkspaceId};

use crate::TermStatus;

#[derive(Clone, Debug)]
pub(crate) enum Note {
    Born { id: TermId, workspace: WorkspaceId },
    Status { id: TermId, status: TermStatus },
    Closed { id: TermId },
}

type Listener = Arc<dyn Fn(&Note) + Send + Sync>;

/// Listeners must not block and must not lock a terminal.
#[derive(Default)]
pub(crate) struct Hub {
    listeners: Mutex<Vec<(u64, Listener)>>,
    next: Mutex<u64>,
}

impl Hub {
    pub(crate) fn subscribe(&self, listener: Box<dyn Fn(&Note) + Send + Sync>) -> u64 {
        let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
        *next += 1;
        let id = *next;
        self.listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((id, Arc::from(listener)));
        id
    }

    pub(crate) fn unsubscribe(&self, id: u64) {
        self.listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|(i, _)| *i != id);
    }

    pub(crate) fn emit(&self, notes: &[Note]) {
        // Call without the lock: a listener may take its own locks, and another thread
        // may hold one of those while it emits.
        let listeners: Vec<Listener> = self
            .listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(_, l)| Arc::clone(l))
            .collect();
        for note in notes {
            for listener in &listeners {
                listener(note);
            }
        }
    }
}
