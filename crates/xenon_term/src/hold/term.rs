use std::fs::File;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use xenon_core::WorkspaceId;

use super::claims::Claims;
use super::emu::Emu;
use super::link::{Link, SLOW_LIMIT};
use super::pty;
use super::status::Status;
use crate::Claim;
use crate::proto::v1::FromHolder;

pub(crate) const HANGUP_GRACE: Duration = Duration::from_secs(5);

pub(crate) struct Peer {
    pub link: Link,
    pub viewing: bool,
    pub answers: bool,
    pub desk: bool,
    pub dropped: bool,
}

pub(crate) struct State {
    pub emu: Emu,
    pub claims: Claims,
    pub status: Status,
    pub peers: Vec<Peer>,
    pub closed_at: Option<Instant>,
    pub closed: bool,
}

pub(crate) struct Holder {
    pub workspace: WorkspaceId,
    pub pid: i32,
    pub cwd: String,
    pub master: File,
    write: Mutex<()>,
    state: Mutex<State>,
}

impl Holder {
    pub(crate) fn new(
        workspace: WorkspaceId,
        pid: i32,
        cwd: String,
        master: File,
        emu: Emu,
    ) -> Self {
        let spawn_dir = std::path::Path::new(&cwd)
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        Self {
            workspace,
            pid,
            cwd,
            master,
            write: Mutex::new(()),
            state: Mutex::new(State {
                emu,
                claims: Claims::default(),
                status: Status::new(spawn_dir),
                peers: Vec::new(),
                closed_at: None,
                closed: false,
            }),
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn write(&self, bytes: &[u8]) {
        let _one_at_a_time = self.write.lock().unwrap_or_else(|e| e.into_inner());
        let _ = pty::write_all(&self.master, bytes);
    }

    pub(crate) fn join(&self, link: Link) {
        let mut st = self.lock();
        link.send(&FromHolder::Status(st.status.snapshot()));
        st.peers.push(Peer {
            link,
            viewing: false,
            answers: false,
            desk: false,
            dropped: false,
        });
    }

    pub(crate) fn view(&self, conn: u64, answers: bool) {
        let mut st = self.lock();
        let size = st.emu.size;
        let replay = st.emu.replay();
        let Some(peer) = st.peer(conn) else { return };
        peer.link.send(&FromHolder::Size(size));
        send_screen(&peer.link, replay);
        peer.viewing = true;
        peer.answers = answers;
        peer.desk |= answers;
        peer.dropped = false;
    }

    pub(crate) fn claim(&self, conn: u64, claim: Claim) {
        let mut st = self.lock();
        let from_desk = matches!(claim, Claim::Desk(_));
        if let Some(peer) = st.peer(conn) {
            peer.desk |= from_desk;
        }
        st.claims.set(conn, claim);
        st.apply_size(&self.master, Instant::now(), from_desk.then_some(conn));
        st.sync_label(Instant::now());
    }

    pub(crate) fn leave(&self, conn: u64) {
        let mut st = self.lock();
        st.peers.retain(|p| p.link.conn != conn);
        st.claims.drop_conn(conn);
        let now = Instant::now();
        st.apply_size(&self.master, now, None);
        st.sync_label(now);
    }

    pub(crate) fn input(&self, conn: u64, bytes: &[u8]) {
        self.write(bytes);
        let now = Instant::now();
        let mut st = self.lock();
        let desk = st.peer(conn).is_some_and(|p| p.desk);
        let mut changed = st.status.on_input();
        if desk && st.claims.note_desk_input(now) {
            st.apply_size(&self.master, now, None);
            changed = true;
        }
        let label = st.claims.active_label(now);
        changed |= st.status.set_phone(label);
        if changed {
            st.broadcast_status();
        }
    }

    pub(crate) fn seen(&self) {
        let mut st = self.lock();
        if st.status.seen() {
            st.broadcast_status();
        }
    }

    pub(crate) fn close(&self) -> bool {
        let mut st = self.lock();
        let exited = st.status.snapshot().exit.is_some();
        if !exited {
            st.closed_at.get_or_insert_with(Instant::now);
            st.closed = true;
            pty::hangup(self.pid);
        }
        exited
    }
}

fn send_screen(link: &Link, replay: Vec<u8>) {
    link.send(&FromHolder::Screen);
    link.output(replay);
}

impl State {
    fn peer(&mut self, conn: u64) -> Option<&mut Peer> {
        self.peers.iter_mut().find(|p| p.link.conn == conn)
    }

    pub(crate) fn broadcast_status(&mut self) {
        let msg = FromHolder::Status(self.status.snapshot());
        self.peers.retain(|p| p.link.send(&msg));
    }

    pub(crate) fn sync_label(&mut self, now: Instant) {
        let label = self.claims.active_label(now);
        if self.status.set_phone(label) {
            self.broadcast_status();
        }
    }

    pub(crate) fn apply_size(&mut self, master: &File, now: Instant, from_desk: Option<u64>) {
        let size = self.claims.effective(now, self.emu.size);
        if size == self.emu.size {
            return;
        }
        self.emu.resize(size);
        pty::set_size(master, size);
        let replay = self.emu.replay();
        for peer in self.peers.iter_mut().filter(|p| p.viewing) {
            peer.link.send(&FromHolder::Size(size));
            if from_desk != Some(peer.link.conn) {
                send_screen(&peer.link, replay.clone());
            }
        }
    }

    pub(crate) fn forward(&mut self, bytes: &[u8]) {
        let mut replay = None;
        let emu = &mut self.emu;
        self.peers.retain_mut(|peer| {
            if !peer.viewing {
                return true;
            }
            let queued = peer.link.queued();
            if peer.dropped && queued == 0 {
                peer.dropped = false;
                let replay = replay.get_or_insert_with(|| emu.replay()).clone();
                send_screen(&peer.link, replay);
                true
            } else if peer.dropped || bytes.is_empty() {
                true
            } else if queued > SLOW_LIMIT {
                peer.dropped = true;
                true
            } else {
                peer.link.output(bytes.to_vec())
            }
        });
    }

    pub(crate) fn has_answerer(&self) -> bool {
        self.peers.iter().any(|p| p.viewing && p.answers)
    }

    pub(crate) fn has_dropped(&self) -> bool {
        self.peers.iter().any(|p| p.dropped)
    }
}
