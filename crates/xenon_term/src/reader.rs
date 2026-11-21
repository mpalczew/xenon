//! Each terminal's own thread: reads the master, feeds the emulator and the window,
//! and runs the timers (quiet, a phone's wait for the desk, a slow window, a close).

use std::fs::File;
use std::io::{self, Read};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::hub::{Hub, Note};
use crate::pty::{self, Pty};
use crate::term::{HANGUP_GRACE, Term};

/// A window that fell behind is checked this often for catching up.
const SLOW_RECHECK: Duration = Duration::from_millis(250);

pub(crate) fn run(term: &Arc<Term>, pty: Pty, hub: &Arc<Hub>) -> io::Result<()> {
    let Pty {
        master,
        mut child,
        pidfd,
    } = pty;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let wait = next_wait(term, Instant::now());
        let mut fds = [
            pty::pollfd(&master, libc::POLLIN),
            pty::pollfd(&pidfd, libc::POLLIN),
        ];
        pty::poll(&mut fds, wait)?;
        let (readable, exited) = (fds[0].revents != 0, fds[1].revents != 0);
        if readable || exited {
            // After an exit, what the child wrote is still in the kernel's buffer.
            let eof = drain(&master, &mut buf, term, hub);
            if eof || exited {
                break;
            }
        }
        hub.emit(&tick(term, Instant::now()));
    }
    let exit = child.wait().map_or(crate::Exit::Code(-1), pty::exit_of);
    let note = {
        let mut st = term.lock();
        st.status.on_exit(exit);
        st.status_note(term.id)
    };
    hub.emit(&[note]);
    Ok(())
}

/// Read until nothing is left. True at end of file (every slave end is closed).
fn drain(master: &File, buf: &mut [u8], term: &Term, hub: &Hub) -> bool {
    let mut file = master;
    loop {
        match file.read(buf) {
            Ok(0) => return true,
            Ok(n) => output(term, hub, &buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return false,
            Err(_) => return true,
        }
    }
}

fn output(term: &Term, hub: &Hub, bytes: &[u8]) {
    let now = Instant::now();
    let (replies, notes) = {
        let mut st = term.lock();
        let advance = st.emu.advance(bytes);
        st.forward(term.id, bytes);
        for watch in &st.watchers {
            watch.poke();
        }
        let mut changed = st.status.on_output(now);
        changed |= advance.bell && st.status.on_bell();
        let raw = st.emu.title.clone();
        changed |= st.status.set_title(raw.as_deref());
        let notes = if changed {
            vec![st.status_note(term.id)]
        } else {
            Vec::new()
        };
        // One answerer: the window, when there is one.
        let replies = if st.desk.is_some() {
            Vec::new()
        } else {
            advance.replies
        };
        (replies, notes)
    };
    if !replies.is_empty() {
        term.write(&replies);
    }
    hub.emit(&notes);
}

/// Milliseconds until a timer is due, or -1 when none is.
fn next_wait(term: &Term, now: Instant) -> i32 {
    let st = term.lock();
    let mut due: Vec<Instant> = Vec::new();
    due.extend(st.status.quiet_deadline());
    due.extend(st.claims.wait_left(now).map(|d| now + d));
    due.extend(st.closed_at.map(|at| at + HANGUP_GRACE));
    due.extend(
        st.desk
            .as_ref()
            .filter(|d| d.dropped)
            .map(|_| now + SLOW_RECHECK),
    );
    due.into_iter().min().map_or(-1, |at| {
        at.saturating_duration_since(now).as_millis() as i32 + 1
    })
}

fn tick(term: &Term, now: Instant) -> Vec<Note> {
    let mut st = term.lock();
    let mut changed = st.status.on_quiet(now);
    st.resize_to_claims(&term.master, now, term.id);
    changed |= st.sync_phone_label(now);
    if st.desk.as_ref().is_some_and(|d| d.dropped) {
        st.forward(term.id, &[]);
    }
    if st.closed_at.is_some_and(|at| now >= at + HANGUP_GRACE) {
        st.closed_at = None;
        unsafe { libc::kill(-term.pid, libc::SIGKILL) };
    }
    if changed {
        vec![st.status_note(term.id)]
    } else {
        Vec::new()
    }
}
