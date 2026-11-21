//! One terminal: its screen, claims, status, the attached window and watching phones.
//! Methods that change status return the notes to emit; the caller emits them after the
//! lock is released.

use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use xenon_core::{TermId, WorkspaceId};

use crate::claims::Claims;
use crate::conn::{Link, SLOW_LIMIT};
use crate::emu::Emu;
use crate::hub::Note;
use crate::pty;
use crate::status::Status;
use crate::wire::Message;
use crate::{Claim, Size, TermInfo};

/// A terminal that was closed gets this long to hang up before it is killed.
pub(crate) const HANGUP_GRACE: Duration = Duration::from_secs(5);

static SERIAL: AtomicU64 = AtomicU64::new(1);

/// The one attached window. It answers terminal queries, so the emulator's replies are
/// dropped while it is set.
pub(crate) struct Desk {
    pub link: Link,
    /// Fell too far behind: no output is sent until it catches up, then it is repainted.
    pub dropped: bool,
}

pub(crate) struct State {
    pub emu: Emu,
    pub claims: Claims,
    pub status: Status,
    pub desk: Option<Desk>,
    pub watchers: Vec<Arc<crate::phone::Watch>>,
    pub closed_at: Option<Instant>,
}

pub(crate) struct Term {
    pub id: TermId,
    pub workspace: WorkspaceId,
    /// Phone tabs are numbered, not named by the long id.
    pub serial: u64,
    pub pid: i32,
    pub cwd: PathBuf,
    pub master: File,
    write: Mutex<()>,
    state: Mutex<State>,
}

impl Term {
    pub(crate) fn new(
        workspace: WorkspaceId,
        pid: i32,
        cwd: PathBuf,
        master: File,
        size: Size,
    ) -> Self {
        let spawn_dir = cwd
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        Self {
            id: TermId::new(),
            workspace,
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            pid,
            cwd,
            master,
            write: Mutex::new(()),
            state: Mutex::new(State {
                emu: Emu::new(size),
                claims: Claims::default(),
                status: Status::new(spawn_dir),
                desk: None,
                watchers: Vec::new(),
                closed_at: None,
            }),
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn info(&self) -> TermInfo {
        TermInfo {
            id: self.id,
            workspace: self.workspace,
            pid: self.pid,
            status: self.lock().status.snapshot(),
        }
    }

    pub(crate) fn write(&self, bytes: &[u8]) {
        let _one_at_a_time = self.write.lock().unwrap_or_else(|e| e.into_inner());
        let _ = pty::write_all(&self.master, bytes);
    }

    /// Replay first, then live, under one lock: no byte is lost or sent twice.
    pub(crate) fn attach(&self, link: Link, size: Size) -> Vec<Note> {
        let mut st = self.lock();
        let mut notes = st.claim(&self.master, Claim::Desk(size), link.conn, self.id);
        let replay = st.emu.replay();
        link.send(Message::Size {
            id: self.id,
            size: st.emu.size,
        });
        link.output(self.id, replay);
        link.send(Message::Status {
            id: self.id,
            status: st.status.snapshot(),
        });
        st.desk = Some(Desk {
            link,
            dropped: false,
        });
        notes.extend(st.sized_notes(self.id));
        notes
    }

    pub(crate) fn claim(&self, conn: u64, claim: Claim) -> Vec<Note> {
        let mut st = self.lock();
        let mut notes = st.claim(&self.master, claim, conn, self.id);
        notes.extend(st.sized_notes(self.id));
        notes
    }

    /// A connection went away: it is no longer the desk, and its claims lapse.
    pub(crate) fn detach(&self, conn: u64) -> Vec<Note> {
        let mut st = self.lock();
        if st.desk.as_ref().is_some_and(|d| d.link.conn == conn) {
            st.desk = None;
        }
        st.claims.drop_conn(conn);
        st.resize_to_claims(&self.master, Instant::now(), self.id);
        st.sized_notes(self.id)
    }

    /// Bytes for the child. `desk` input also takes the size back from a phone.
    pub(crate) fn input(&self, bytes: &[u8], desk: bool) -> Vec<Note> {
        self.write(bytes);
        let now = Instant::now();
        let mut st = self.lock();
        let mut changed = st.status.on_input();
        if desk && st.claims.note_desk_input(now) {
            st.resize_to_claims(&self.master, now, self.id);
            changed = true;
        }
        changed |= st.sync_phone_label(now);
        if changed {
            vec![st.status_note(self.id)]
        } else {
            Vec::new()
        }
    }

    pub(crate) fn seen(&self) -> Vec<Note> {
        let mut st = self.lock();
        if st.status.seen() {
            vec![st.status_note(self.id)]
        } else {
            Vec::new()
        }
    }

    pub(crate) fn mark_closed(&self) {
        self.lock().closed_at = Some(Instant::now());
    }
}

impl State {
    pub(crate) fn status_note(&self, id: TermId) -> Note {
        Note::Status {
            id,
            status: self.status.snapshot(),
        }
    }

    /// Status notes that follow from a claim change or a timer.
    fn sized_notes(&mut self, id: TermId) -> Vec<Note> {
        if self.sync_phone_label(Instant::now()) {
            vec![self.status_note(id)]
        } else {
            Vec::new()
        }
    }

    pub(crate) fn sync_phone_label(&mut self, now: Instant) -> bool {
        let label = self.claims.active_label(now);
        self.status.set_phone(label)
    }

    fn claim(&mut self, master: &File, claim: Claim, conn: u64, id: TermId) -> Vec<Note> {
        let from_desk = matches!(claim, Claim::Desk(_));
        self.claims.set(conn, claim);
        self.apply_size(master, Instant::now(), id, from_desk);
        Vec::new()
    }

    /// The claims changed or the quiet period ended: resize if the merge says so.
    pub(crate) fn resize_to_claims(&mut self, master: &File, now: Instant, id: TermId) {
        self.apply_size(master, now, id, false);
    }

    /// Resize emulator and PTY to what the claims merge to. The window is told first,
    /// and repainted when the size change came from a phone: its own reflow may differ.
    fn apply_size(&mut self, master: &File, now: Instant, id: TermId, from_desk: bool) {
        let size = self.claims.effective(now, self.emu.size);
        if size == self.emu.size {
            return;
        }
        self.emu.resize(size);
        pty::set_size(master, size);
        if let Some(desk) = &self.desk {
            desk.link.send(Message::Size { id, size });
            if !from_desk {
                let replay = self.emu.replay();
                desk.link.output(id, replay);
            }
        }
        for watch in &self.watchers {
            watch.poke();
        }
    }

    /// Send program output to the window, or drop and repaint a window that is too slow.
    pub(crate) fn forward(&mut self, id: TermId, bytes: &[u8]) {
        let Some(desk) = self.desk.as_mut() else {
            return;
        };
        let queued = desk.link.queued();
        let sent = if desk.dropped && queued == 0 {
            desk.dropped = false;
            let replay = self.emu.replay();
            desk.link.output(id, replay)
        } else if desk.dropped {
            true
        } else if queued > SLOW_LIMIT {
            desk.dropped = true;
            true
        } else {
            desk.link.output(id, bytes.to_vec())
        };
        if !sent {
            self.desk = None;
        }
    }
}
