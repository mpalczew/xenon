//! Where the GTK frontend keeps its state, and how that state is seeded.
//!
//! The GPUI app writes whole files (`workspaces.json`, `sessions/*.json`,
//! `settings.json`) from its in-memory copy, with no cross-process lock and no
//! change watching. Two live writers on one directory would overwrite each
//! other. The A/B slot launcher already solves this by giving each instance its
//! own directory and copying state across. The GTK app follows that precedent:
//! it owns `~/.xenon-gtk` (same file formats) and seeds it once from the GPUI
//! slot that was active last. It never writes into a GPUI directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Files that belong to a live instance and are never copied.
const RUNTIME_FILES: [&str; 3] = ["xenon.pid", "ipc.sock", "xenon.log"];

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// `XENON_GTK_DATA_DIR`, else `~/.xenon-gtk`.
pub(crate) fn state_dir() -> PathBuf {
    std::env::var_os("XENON_GTK_DATA_DIR").map_or_else(|| home().join(".xenon-gtk"), PathBuf::from)
}

/// The GPUI directory to seed from: the slot used last, else the ship dir.
fn seed_source() -> Option<PathBuf> {
    let home = home();
    let slot = fs::read_to_string(home.join(".xenon-active-slot")).unwrap_or_default();
    let slot_dir = match slot.trim() {
        "a" => Some(home.join(".xenon-a")),
        "b" => Some(home.join(".xenon-b")),
        _ => None,
    };
    slot_dir
        .filter(|dir| dir.join("workspaces.json").exists())
        .or_else(|| Some(home.join(".xenon")).filter(|dir| dir.join("workspaces.json").exists()))
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if RUNTIME_FILES.iter().any(|skip| name == *skip) {
            continue;
        }
        let to = dst.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Make sure `dir` holds state. With `reseed`, an existing directory is moved
/// aside first. Returns a line describing what happened, for the log.
pub(crate) fn prepare(dir: &Path, reseed: bool) -> String {
    if reseed && dir.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let aside = dir.with_extension(format!("bak-{stamp}"));
        if let Err(error) = fs::rename(dir, &aside) {
            return format!("could not move {} aside: {error}", dir.display());
        }
    }
    if dir.join("workspaces.json").exists() {
        return format!("state: {}", dir.display());
    }
    let Some(source) = seed_source() else {
        return format!("state: {} (new, nothing to seed from)", dir.display());
    };
    match copy_tree(&source, dir) {
        Ok(()) => format!("state: {} seeded from {}", dir.display(), source.display()),
        Err(error) => format!("seeding {} failed: {error}", dir.display()),
    }
}

/// Where the GPUI app keeps `remote_devices.json`, so a phone paired there is
/// paired here with no copy to drift: `~/.xenon-shared` (the a/b slot
/// launcher's shared dir), else the seed source directory (a plain `~/.xenon`).
/// None when `XENON_SHARED_DIR` is already set or the GPUI app has no such
/// file; the GTK app then keeps its own in its state dir.
pub(crate) fn shared_remote_dir() -> Option<PathBuf> {
    if std::env::var_os("XENON_SHARED_DIR").is_some() {
        return None;
    }
    let has = |dir: &PathBuf| dir.join("remote_devices.json").exists();
    Some(home().join(".xenon-shared"))
        .filter(has)
        .or_else(|| seed_source().filter(has))
}
