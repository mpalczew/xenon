//! A child on a pseudo-terminal: spawn, resize, write, and a handle that becomes
//! readable when the child exits.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::{Exit, Size};

pub(crate) struct Pty {
    /// Non-blocking.
    pub master: File,
    pub child: Child,
    /// Readable once the child has exited.
    pub pidfd: OwnedFd,
}

fn winsize(size: Size) -> libc::winsize {
    libc::winsize {
        ws_row: size.rows(),
        ws_col: size.cols(),
        ws_xpixel: 0,
        ws_ypixel: 0,
    }
}

fn check(rc: libc::c_int) -> io::Result<libc::c_int> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc)
    }
}

/// Run `argv` (the user's shell when empty) in `cwd` with exactly `env`.
pub(crate) fn spawn(
    cwd: &Path,
    argv: &[String],
    env: &HashMap<String, String>,
    size: Size,
) -> io::Result<Pty> {
    let (mut master, mut slave) = (0, 0);
    let ws = winsize(size);
    check(unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            &ws,
        )
    })?;
    // SAFETY: `openpty` returned two fresh descriptors that nothing else owns.
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    for fd in [&master, &slave] {
        check(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) })?;
    }
    let shell = || {
        env.get("SHELL")
            .cloned()
            .unwrap_or_else(|| "/bin/sh".into())
    };
    let (program, args) = match argv.split_first() {
        Some((program, args)) => (program.clone(), args),
        None => (shell(), &[][..]),
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .envs(env)
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave));
    if !cwd.as_os_str().is_empty() {
        command.current_dir(cwd);
    }
    // SAFETY: only async-signal-safe calls (`setsid`, `ioctl`) run between fork and exec.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            libc::ioctl(0, libc::TIOCSCTTY, 0);
            Ok(())
        });
    }
    let child = command.spawn()?;
    // SAFETY: `pidfd_open` takes a pid and flags and returns a new descriptor or -1.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, child.id() as libc::pid_t, 0) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the syscall returned a fresh descriptor.
    let pidfd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
    check(unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) })?;
    Ok(Pty {
        master: File::from(master),
        child,
        pidfd,
    })
}

pub(crate) fn set_size(master: &File, size: Size) {
    let ws = winsize(size);
    unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &ws) };
}

/// Hang up the child's session.
pub(crate) fn hangup(pid: i32) {
    unsafe { libc::kill(-pid, libc::SIGHUP) };
}

/// Wait for `fd` to be ready for `events`, up to `timeout_ms` (-1 forever).
/// Returns the `revents` of each descriptor.
pub(crate) fn poll(fds: &mut [libc::pollfd], timeout_ms: i32) -> io::Result<()> {
    loop {
        let rc = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms) };
        if rc >= 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

pub(crate) fn pollfd(file: &impl AsRawFd, events: i16) -> libc::pollfd {
    libc::pollfd {
        fd: file.as_raw_fd(),
        events,
        revents: 0,
    }
}

/// The master is non-blocking: wait for room when the child isn't reading.
pub(crate) fn write_all(master: &File, mut bytes: &[u8]) -> io::Result<()> {
    let mut out = master;
    while !bytes.is_empty() {
        match out.write(bytes) {
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                poll(&mut [pollfd(master, libc::POLLOUT)], 1000)?;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub(crate) fn exit_of(status: std::process::ExitStatus) -> Exit {
    match (status.code(), status.signal()) {
        (Some(code), _) => Exit::Code(code),
        (None, Some(signal)) => Exit::Signal(signal),
        (None, None) => Exit::Code(-1),
    }
}
