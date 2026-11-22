mod claims;
mod emu;
mod emu_replay;
mod link;
mod pen;
pub(crate) mod pty;
mod reader;
mod serve;
mod status;
mod term;

use std::collections::HashMap;
use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;

use xenon_core::TermId;

pub(crate) use emu::Emu;
pub use emu::Listener;

use term::Holder;

use crate::start::{Spec, take_spec};
use crate::{Size, dir};

pub(crate) const FIRST_SIZE: Size = Size { cols: 80, rows: 24 };

static SOCKET: OnceLock<CString> = OnceLock::new();

pub fn run_holder(id: TermId) -> ! {
    if let Err(error) = run(id) {
        eprintln!("xenon-term hold {id}: {error}");
        std::process::exit(1);
    }
    exit_now()
}

pub(crate) fn exit_now() -> ! {
    if let Some(path) = SOCKET.get() {
        // SAFETY: `unlink` on a valid C string; also what the signal handler does.
        unsafe { libc::unlink(path.as_ptr()) };
    }
    std::process::exit(0)
}

extern "C" fn on_signal(_: libc::c_int) {
    if let Some(path) = SOCKET.get() {
        // SAFETY: `unlink` and `_exit` are async-signal-safe.
        unsafe { libc::unlink(path.as_ptr()) };
    }
    unsafe { libc::_exit(0) };
}

fn run(id: TermId) -> io::Result<()> {
    let spec = take_spec(id)?;
    let settings = xenon_store::load_service_settings().unwrap_or_default();
    let (holder, pty) = start_terminal(&spec, settings.terminal_scrollback_lines)?;
    let holder = Arc::new(holder);
    let listener = bind(id)?;
    let reader = Arc::clone(&holder);
    thread::Builder::new()
        .name("xenon-term-reader".into())
        .spawn(move || {
            if let Err(error) = reader::run(&reader, pty) {
                log::warn!("terminal reader: {error}");
            }
            if reader.lock().closed {
                exit_now();
            }
        })?;
    if let Err(error) = sd_notify::notify(false, &[sd_notify::NotifyState::Ready]) {
        log::warn!("notify ready: {error}");
    }
    let next_conn = AtomicU64::new(1);
    for stream in listener.incoming() {
        let stream = stream?;
        let conn = next_conn.fetch_add(1, Ordering::Relaxed);
        let holder = Arc::clone(&holder);
        thread::spawn(move || {
            let _ = serve::serve(&holder, conn, stream);
        });
    }
    Ok(())
}

fn start_terminal(spec: &Spec, scrollback: usize) -> io::Result<(Holder, pty::Pty)> {
    let mut vars: HashMap<String, String> = HashMap::new();
    let workspace = spec.workspace.unwrap_or_default();
    let extra = [
        format!("XENON_WORKSPACE={workspace}"),
        "TERM=xterm-256color".into(),
        "COLORTERM=truecolor".into(),
    ];
    for pair in spec.env.iter().chain(extra.iter()) {
        if let Some((k, v)) = pair.split_once('=') {
            vars.insert(k.to_owned(), v.to_owned());
        }
    }
    let cwd = Path::new(&spec.cwd);
    let live = match pty::spawn(cwd, &spec.argv, &vars, FIRST_SIZE) {
        Ok(live) => live,
        Err(error) => {
            let what = spec.argv.first().map_or("the shell", String::as_str);
            vars.insert(
                "XENON_ERROR".into(),
                format!("xenon: could not start {what}: {error}"),
            );
            let stand_in = [
                "/bin/sh",
                "-c",
                "printf '%s\\r\\n' \"$XENON_ERROR\"; exit 127",
            ]
            .map(String::from);
            pty::spawn(Path::new("/"), &stand_in, &vars, FIRST_SIZE)?
        }
    };
    let pid = live.child.id() as i32;
    let master = live.master.try_clone()?;
    let holder = Holder::new(
        workspace,
        pid,
        spec.cwd.clone(),
        master,
        Emu::new(FIRST_SIZE, scrollback),
    );
    Ok((holder, live))
}

fn bind(id: TermId) -> io::Result<UnixListener> {
    let path = dir::sock_path(id);
    let temp = path.with_extension("sock.tmp");
    let _ = std::fs::remove_file(&temp);
    let listener = UnixListener::bind(&temp)?;
    std::fs::rename(&temp, &path)?;
    if let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) {
        let _ = SOCKET.set(c_path);
    }
    for signal in [libc::SIGHUP, libc::SIGTERM] {
        // SAFETY: the handler only calls async-signal-safe functions.
        unsafe { libc::signal(signal, on_signal as *const () as libc::sighandler_t) };
    }
    Ok(listener)
}
