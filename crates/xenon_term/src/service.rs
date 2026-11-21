//! The service: holds every terminal and talks to windows over a Unix socket that
//! systemd's user manager owns, so the first connect starts it.

use std::io;
use std::os::fd::FromRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use crate::conn::Link;
use crate::hub::Note;
use crate::phone::Phone;
use crate::terms::Terms;
use crate::wire::{self, Listed, Message, Request};
use crate::{Claim, Spawn, socket_path};

/// Run the service until the process is killed. Under systemd socket activation the
/// listening socket arrives as file descriptor 3; otherwise it binds `XENON_TERM_SOCKET`
/// (or the default path) itself, which is how development and checks run a private one.
pub fn run_service() -> io::Result<()> {
    let listener = match activated() {
        Some(listener) => listener,
        None => bind(&socket_path())?,
    };
    let terms = Terms::new();
    let phone = Phone::start(&terms);
    let next_conn = AtomicU64::new(1);
    for stream in listener.incoming() {
        let stream = stream?;
        let conn = next_conn.fetch_add(1, Ordering::Relaxed);
        let (terms, phone) = (Arc::clone(&terms), Arc::clone(&phone));
        thread::spawn(move || {
            let _ = serve(&terms, &phone, conn, stream);
            terms.detach_conn(conn);
        });
    }
    Ok(())
}

/// The socket systemd handed us, if it did (`LISTEN_PID` names this process).
fn activated() -> Option<UnixListener> {
    let var = |name: &str| std::env::var(name).ok()?.parse::<u32>().ok();
    if var("LISTEN_PID")? != std::process::id() || var("LISTEN_FDS")? < 1 {
        return None;
    }
    // SAFETY: systemd passes the listening socket as descriptor 3 (SD_LISTEN_FDS_START)
    // and nothing else in this process has taken it.
    Some(unsafe { UnixListener::from_raw_fd(3) })
}

fn bind(path: &Path) -> io::Result<UnixListener> {
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "terminal service already running",
            ));
        }
        std::fs::remove_file(path)?;
    }
    UnixListener::bind(path)
}

/// Tell this window about everything that happens to any terminal.
fn forward(link: &Link, note: &Note) {
    let msg = match note {
        Note::Born { id, workspace } => Message::Born {
            id: *id,
            workspace: *workspace,
        },
        Note::Status { id, status } => Message::Status {
            id: *id,
            status: status.clone(),
        },
        Note::Closed { id } => Message::Closed { id: *id },
    };
    link.send(msg);
}

fn serve(terms: &Arc<Terms>, phone: &Arc<Phone>, conn: u64, stream: UnixStream) -> io::Result<()> {
    let link = Link::start(conn, stream.try_clone()?);
    let note_link = link.clone();
    let notes = terms
        .hub
        .subscribe(Box::new(move |note| forward(&note_link, note)));
    let phone_link = link.clone();
    let changes = phone.on_change(Box::new(move || {
        phone_link.send(Message::PhoneChanged);
    }));
    let mut input = stream;
    let result = loop {
        match wire::read_request(&mut input) {
            Ok((seq, req)) => {
                if let Some(reply) = handle(terms, phone, &link, seq, req) {
                    link.send(reply);
                }
            }
            Err(e) => break Err(e),
        }
    };
    terms.hub.unsubscribe(notes);
    phone.off_change(changes);
    result
}

fn handle(
    terms: &Arc<Terms>,
    phone: &Phone,
    link: &Link,
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
            terms.set_env(env);
            Some(Message::Ok(seq))
        }
        Request::List => Some(Message::Listed(
            seq,
            terms
                .list()
                .into_iter()
                .map(|t| Listed {
                    id: t.id,
                    workspace: t.workspace,
                    pid: t.pid,
                    status: t.status,
                })
                .collect(),
        )),
        Request::Create {
            workspace,
            cwd,
            argv,
            env,
        } => {
            let spawn = Spawn {
                cwd: cwd.into(),
                argv,
                env,
            };
            Some(match terms.create(workspace, &spawn, None) {
                Ok(id) => Message::Created(seq, id),
                Err(e) => Message::Err(seq, e.to_string()),
            })
        }
        Request::Phone { json } => Some(Message::Reply(seq, phone.call(&json))),
        other => {
            handle_quiet(terms, link, other);
            None
        }
    }
}

/// Requests that get no answer.
fn handle_quiet(terms: &Terms, link: &Link, req: Request) {
    match req {
        Request::Attach { id, size } => {
            terms.attach(id, link.clone(), size);
        }
        Request::Input { id, bytes } => terms.input(id, &bytes, true),
        Request::Claim { id, size } => terms.claim(id, link.conn, Claim::Desk(size)),
        Request::Seen { id } => terms.seen(id),
        Request::Close { id } => terms.close(id),
        Request::Dark { dark } => terms.dark.store(dark, Ordering::Relaxed),
        _ => {}
    }
}
