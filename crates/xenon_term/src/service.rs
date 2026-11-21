//! The service: holds every terminal, reads each PTY on its own thread, and talks to
//! windows over a Unix socket.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use alacritty_terminal::event::WindowSize;
use alacritty_terminal::tty::{self, Options, Pty, Shell};

use crate::emu::Emu;
use crate::wire::{self, Listed, Message, Request};
use crate::{Exit, Size, TermId};

/// Lock, ignoring poison: a panicking terminal thread must not take the service down.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct Desk {
    conn: u64,
    tx: Sender<Message>,
}

struct State {
    emu: Emu,
    /// Owns the child; dropping it hangs up the shell.
    pty: Option<Pty>,
    /// The one attached window. It answers terminal queries, so the emulator's replies
    /// are dropped while it is set.
    desk: Option<Desk>,
    exit: Option<Exit>,
}

struct Term {
    id: TermId,
    pid: i32,
    master: File,
    state: Mutex<State>,
}

#[derive(Default)]
struct Terms {
    base_env: Mutex<Vec<String>>,
    terms: Mutex<HashMap<TermId, Arc<Term>>>,
    next_conn: AtomicU64,
}

/// Run the service on `path` until the process is killed.
///
/// Not implemented in the spike: systemd socket activation (`LISTEN_FDS`), the phone
/// server, size claims, status, and dropping a window that falls too far behind.
pub fn run_service(path: &Path) -> io::Result<()> {
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "service already running",
            ));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    let terms = Arc::new(Terms::default());
    for stream in listener.incoming() {
        let stream = stream?;
        let terms = Arc::clone(&terms);
        let conn = terms.next_conn.fetch_add(1, Ordering::Relaxed);
        thread::spawn(move || {
            let _ = serve_conn(&terms, conn, stream);
            detach(&terms, conn);
        });
    }
    Ok(())
}

fn detach(terms: &Terms, conn: u64) {
    for term in lock(&terms.terms).values() {
        let mut st = lock(&term.state);
        if st.desk.as_ref().is_some_and(|d| d.conn == conn) {
            st.desk = None;
        }
    }
}

fn serve_conn(terms: &Terms, conn: u64, stream: UnixStream) -> io::Result<()> {
    let (tx, rx) = channel::<Message>();
    let mut out = stream.try_clone()?;
    thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            if wire::write_message(&mut out, &msg).is_err() {
                break;
            }
        }
    });
    let mut input = stream;
    loop {
        let (seq, req) = wire::read_request(&mut input)?;
        let reply = handle(terms, conn, &tx, seq, req);
        if let Some(msg) = reply {
            let _ = tx.send(msg);
        }
    }
}

fn handle(
    terms: &Terms,
    conn: u64,
    tx: &Sender<Message>,
    seq: u64,
    req: Request,
) -> Option<Message> {
    match req {
        Request::Hello { version, env } => {
            if version != wire::VERSION {
                return Some(Message::Err(
                    seq,
                    format!("protocol version {version} not supported"),
                ));
            }
            *lock(&terms.base_env) = env;
            Some(Message::Ok(seq))
        }
        Request::List => {
            let items = lock(&terms.terms)
                .values()
                .map(|t| Listed {
                    id: t.id,
                    pid: t.pid,
                    exit: lock(&t.state).exit,
                })
                .collect();
            Some(Message::Listed(seq, items))
        }
        Request::Create {
            cwd,
            argv,
            env,
            size,
        } => Some(match create(terms, &cwd, argv, env, size) {
            Ok(id) => Message::Created(seq, id),
            Err(e) => Message::Err(seq, e.to_string()),
        }),
        Request::Attach { id, size } => {
            let found = lock(&terms.terms).get(&id).cloned();
            match found {
                Some(term) => {
                    attach(&term, conn, tx, size);
                    None
                }
                None => Some(Message::Err(seq, "no such terminal".into())),
            }
        }
        Request::Input { id, bytes } => {
            if let Some(term) = lock(&terms.terms).get(&id).cloned() {
                let _ = write_pty(&term.master, &bytes);
            }
            None
        }
        Request::Resize { id, size } => {
            if let Some(term) = lock(&terms.terms).get(&id).cloned() {
                let mut st = lock(&term.state);
                st.emu.resize(size);
                set_winsize(&term.master, size);
            }
            None
        }
        Request::Close { id } => {
            if let Some(term) = lock(&terms.terms).remove(&id) {
                // SIGHUP the session. The terminal is freed when its reader thread ends.
                unsafe { libc::kill(-term.pid, libc::SIGHUP) };
            }
            None
        }
    }
}

fn create(
    terms: &Terms,
    cwd: &str,
    argv: Vec<String>,
    env: Vec<String>,
    size: Size,
) -> io::Result<TermId> {
    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert("TERM".into(), "xterm-256color".into());
    vars.insert("COLORTERM".into(), "truecolor".into());
    for pair in lock(&terms.base_env).iter().chain(env.iter()) {
        if let Some((k, v)) = pair.split_once('=') {
            vars.insert(k.to_owned(), v.to_owned());
        }
    }
    let shell = argv
        .split_first()
        .map(|(p, args)| Shell::new(p.clone(), args.to_vec()));
    let options = Options {
        shell,
        working_directory: (!cwd.is_empty()).then(|| cwd.into()),
        drain_on_exit: false,
        env: vars,
    };
    let window = WindowSize {
        num_lines: size.rows,
        num_cols: size.cols,
        cell_width: 1,
        cell_height: 1,
    };
    let pty = tty::new(&options, window, 0)?;
    let pid = pty.child().id() as i32;
    let master = pty.file().try_clone()?;
    let reader = pty.file().try_clone()?;
    let id = TermId::random();
    let term = Arc::new(Term {
        id,
        pid,
        master,
        state: Mutex::new(State {
            emu: Emu::new(size),
            pty: Some(pty),
            desk: None,
            exit: None,
        }),
    });
    lock(&terms.terms).insert(id, Arc::clone(&term));
    thread::spawn(move || read_loop(&term, reader));
    Ok(id)
}

/// Replay first, then live, under one lock: no byte is lost or sent twice.
fn attach(term: &Term, conn: u64, tx: &Sender<Message>, size: Size) {
    let mut st = lock(&term.state);
    st.emu.resize(size);
    set_winsize(&term.master, size);
    let bytes = st.emu.replay();
    let _ = tx.send(Message::Output { id: term.id, bytes });
    if let Some(exit) = st.exit {
        let _ = tx.send(Message::Exited { id: term.id, exit });
    }
    st.desk = Some(Desk {
        conn,
        tx: tx.clone(),
    });
}

fn read_loop(term: &Term, mut file: File) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let mut pfd = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut pfd, 1, -1) } < 0
            && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
        {
            break;
        }
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => output(term, &buf[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => break,
        }
    }
    finish(term);
}

fn output(term: &Term, bytes: &[u8]) {
    let replies = {
        let mut st = lock(&term.state);
        let replies = st.emu.advance(bytes);
        let sent = st.desk.as_ref().map(|d| {
            d.tx.send(Message::Output {
                id: term.id,
                bytes: bytes.to_vec(),
            })
            .is_ok()
        });
        if sent == Some(false) {
            st.desk = None;
        }
        // One answerer: the window, when there is one.
        if st.desk.is_some() {
            Vec::new()
        } else {
            replies
        }
    };
    if !replies.is_empty() {
        let _ = write_pty(&term.master, &replies);
    }
}

/// The master hung up: record how the child ended and tell the window.
fn finish(term: &Term) {
    let exit = wait_exit(term.pid);
    let mut st = lock(&term.state);
    st.exit = Some(exit);
    if let Some(d) = &st.desk {
        let _ = d.tx.send(Message::Exited { id: term.id, exit });
    }
    // Dropping the Pty reaps the child.
    st.pty = None;
}

/// Read the exit status without reaping, so the `Pty` drop still can.
fn wait_exit(pid: i32) -> Exit {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOWAIT,
        )
    };
    if rc != 0 {
        return Exit::Code(-1);
    }
    let status = unsafe { info.si_status() };
    if info.si_code == libc::CLD_EXITED {
        Exit::Code(status)
    } else {
        Exit::Signal(status)
    }
}

fn set_winsize(master: &File, size: Size) {
    let ws = libc::winsize {
        ws_row: size.rows,
        ws_col: size.cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &ws) };
}

/// The master is non-blocking: wait for room when the child isn't reading.
fn write_pty(master: &File, mut bytes: &[u8]) -> io::Result<()> {
    let mut w = master;
    while !bytes.is_empty() {
        match w.write(bytes) {
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let mut pfd = libc::pollfd {
                    fd: master.as_raw_fd(),
                    events: libc::POLLOUT,
                    revents: 0,
                };
                unsafe { libc::poll(&mut pfd, 1, 1000) };
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
