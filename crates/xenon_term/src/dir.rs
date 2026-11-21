//! The runtime directory is the registry of terminals:
//!
//! ```text
//! <runtime>/term/<TermId>.sock   a ready holder (renamed into place once its child runs)
//! <runtime>/term/<TermId>.spec   spawn request, mode 0600; the holder reads and deletes it
//! <runtime>/phone.sock           the phone server's control socket
//! ```
//!
//! `<runtime>` is `$XENON_RUNTIME_DIR` (private runs), else `$XDG_RUNTIME_DIR/xenon`.

use std::io;
use std::path::{Path, PathBuf};
use std::thread;

use inotify::{Inotify, WatchMask};
use xenon_core::TermId;

pub(crate) fn runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XENON_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    base.join("xenon")
}

/// Where the phone server listens for the window's calls.
pub fn phone_socket_path() -> PathBuf {
    runtime_dir().join("phone.sock")
}

pub(crate) fn term_dir() -> PathBuf {
    runtime_dir().join("term")
}

pub(crate) fn sock_path(id: TermId) -> PathBuf {
    term_dir().join(format!("{id}.sock"))
}

pub(crate) fn spec_path(id: TermId) -> PathBuf {
    term_dir().join(format!("{id}.spec"))
}

/// Every terminal with a socket in the directory.
pub(crate) fn ids() -> Vec<TermId> {
    let Ok(entries) = std::fs::read_dir(term_dir()) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            name.to_str()?.strip_suffix(".sock")?.parse().ok()
        })
        .collect()
}

/// A socket that refuses connections belongs to a crashed holder: delete it.
/// `ECONNREFUSED` on a Unix socket means nobody listens, so a live holder is never hit.
pub(crate) fn sweep_if_dead(id: TermId, error: &io::Error) {
    if error.kind() == io::ErrorKind::ConnectionRefused {
        let _ = std::fs::remove_file(sock_path(id));
    }
}

/// Call `on_change` whenever something is created, moved into or deleted from `dir`
/// (an overflowed queue counts too). The callback should rescan, not trust the event.
pub fn watch_dir(dir: &Path, on_change: impl Fn() + Send + 'static) -> io::Result<()> {
    let mut inotify = Inotify::init()?;
    inotify.watches().add(
        dir,
        WatchMask::CREATE | WatchMask::MOVED_TO | WatchMask::DELETE,
    )?;
    thread::Builder::new()
        .name("xenon-dir-watch".into())
        .spawn(move || {
            let mut buffer = [0; 4096];
            while let Ok(events) = inotify.read_events_blocking(&mut buffer) {
                events.for_each(drop);
                on_change();
            }
        })?;
    Ok(())
}
