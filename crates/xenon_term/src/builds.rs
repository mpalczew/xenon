//! Install replaces the binary by rename, so an older holder's `/proc/<pid>/exe` is a
//! different inode from the installed one.

use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use crate::start::installed_binary;
use crate::terminals::Shared;
use crate::{Event, dir};

type Ident = (u64, u64);

fn ident(path: impl AsRef<Path>) -> Option<Ident> {
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

pub(crate) fn older(holder_pid: i32) -> bool {
    match (
        ident(format!("/proc/{holder_pid}/exe")),
        ident(installed_binary()),
    ) {
        (Some(running), Some(installed)) => running != installed,
        _ => false,
    }
}

pub(crate) fn watch(shared: &std::sync::Arc<Shared>) {
    let binary = installed_binary();
    let Some(folder) = binary.parent() else {
        return;
    };
    let seen = Mutex::new(ident(&binary));
    let shared = std::sync::Arc::clone(shared);
    let started = dir::watch_dir(folder, move || {
        let now = ident(installed_binary());
        let changed =
            std::mem::replace(&mut *seen.lock().unwrap_or_else(|e| e.into_inner()), now) != now;
        if changed {
            shared.emit(Event::Builds);
        }
    });
    if let Err(error) = started {
        log::warn!("watch for new builds: {error}");
    }
}

static LABEL: Mutex<Option<(Ident, String)>> = Mutex::new(None);

pub fn installed_build() -> String {
    let now = ident(installed_binary());
    let mut cache = LABEL.lock().unwrap_or_else(|e| e.into_inner());
    if let (Some(now), Some((seen, label))) = (now, cache.as_ref())
        && *seen == now
    {
        return label.clone();
    }
    let ran = Command::new(installed_binary()).arg("version").output();
    let label = ran
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| crate::BUILD.to_owned());
    if let Some(now) = now {
        *cache = Some((now, label.clone()));
    }
    label
}
