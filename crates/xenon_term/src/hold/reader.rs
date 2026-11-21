use std::fs::File;
use std::io::{self, Read};
use std::time::{Duration, Instant};

use super::pty::{self, Pty};
use super::term::{HANGUP_GRACE, Holder};

const SLOW_RECHECK: Duration = Duration::from_millis(250);

pub(crate) fn run(holder: &Holder, pty: Pty) -> io::Result<()> {
    let Pty {
        master,
        mut child,
        pidfd,
    } = pty;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let wait = next_wait(holder, Instant::now());
        let mut fds = [
            pty::pollfd(&master, libc::POLLIN),
            pty::pollfd(&pidfd, libc::POLLIN),
        ];
        pty::poll(&mut fds, wait)?;
        let (readable, exited) = (fds[0].revents != 0, fds[1].revents != 0);
        if readable || exited {
            let eof = drain(&master, &mut buf, holder);
            if eof || exited {
                break;
            }
        }
        tick(holder, Instant::now());
    }
    let exit = child.wait().map_or(crate::Exit::Code(-1), pty::exit_of);
    let mut st = holder.lock();
    st.status.on_exit(exit);
    st.broadcast_status();
    Ok(())
}

fn drain(master: &File, buf: &mut [u8], holder: &Holder) -> bool {
    let mut file = master;
    loop {
        match file.read(buf) {
            Ok(0) => return true,
            Ok(n) => output(holder, &buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return false,
            Err(_) => return true,
        }
    }
}

fn output(holder: &Holder, bytes: &[u8]) {
    let now = Instant::now();
    let replies = {
        let mut st = holder.lock();
        let advance = st.emu.advance(bytes);
        st.forward(bytes);
        let mut changed = st.status.on_output(now);
        changed |= advance.bell && st.status.on_bell();
        let raw = st.emu.title.clone();
        changed |= st.status.set_title(raw.as_deref());
        if changed {
            st.broadcast_status();
        }
        if st.has_answerer() {
            Vec::new()
        } else {
            advance.replies
        }
    };
    if !replies.is_empty() {
        holder.write(&replies);
    }
}

fn next_wait(holder: &Holder, now: Instant) -> i32 {
    let st = holder.lock();
    let mut due: Vec<Instant> = Vec::new();
    due.extend(st.status.quiet_deadline());
    due.extend(st.claims.wait_left(now).map(|d| now + d));
    due.extend(st.closed_at.map(|at| at + HANGUP_GRACE));
    if st.has_dropped() {
        due.push(now + SLOW_RECHECK);
    }
    due.into_iter().min().map_or(-1, |at| {
        at.saturating_duration_since(now).as_millis() as i32 + 1
    })
}

fn tick(holder: &Holder, now: Instant) {
    let mut st = holder.lock();
    if st.status.on_quiet(now) {
        st.broadcast_status();
    }
    st.apply_size(&holder.master, now, None);
    st.sync_label(now);
    if st.has_dropped() {
        st.forward(&[]);
    }
    if st.closed_at.is_some_and(|at| now >= at + HANGUP_GRACE) {
        st.closed_at = None;
        // SAFETY: a plain signal to the child's own process group.
        unsafe { libc::kill(-holder.pid, libc::SIGKILL) };
    }
}
