use std::path::{Path, PathBuf};

const MAX_NAME_BYTES: usize = 255;

pub fn check_name(dir: &Path, typed: &str, current: Option<&str>) -> Result<String, String> {
    let name = typed.trim();
    if name.is_empty() {
        return Err("Enter a name.".into());
    }
    if name.contains('/') {
        return Err("A name can't contain a slash (/).".into());
    }
    if name == "." || name == ".." || name.contains('\0') {
        return Err("That name isn't allowed.".into());
    }
    if name.len() > MAX_NAME_BYTES {
        return Err("That name is too long.".into());
    }
    if current == Some(name) {
        return Err("Enter a different name.".into());
    }
    if dir.join(name).symlink_metadata().is_ok() {
        return Err(format!(
            "\u{201c}{name}\u{201d} already exists in this folder."
        ));
    }
    Ok(name.to_string())
}

/// How many characters of `name` come before its extension, for selecting
/// just the stem when renaming. A leading dot is not an extension.
pub fn stem_chars(name: &str) -> usize {
    match name.rfind('.') {
        Some(0) | None => name.chars().count(),
        Some(at) => name[..at].chars().count(),
    }
}

pub fn new_item_dir(path: &Path, is_dir: bool) -> PathBuf {
    if is_dir {
        path.to_path_buf()
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    }
}

/// Where `src` ends up when dropped in `dest_dir`. `Ok(None)` means it is
/// already there, so there is nothing to do.
pub fn move_target(src: &Path, dest_dir: &Path) -> Result<Option<PathBuf>, String> {
    if dest_dir.starts_with(src) {
        return Err("A folder can't be moved into itself.".into());
    }
    if src.parent() == Some(dest_dir) {
        return Ok(None);
    }
    landing(src, dest_dir).map(Some)
}

pub fn copy_target(src: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    if dest_dir.starts_with(src) {
        return Err("A folder can't be copied into itself.".into());
    }
    landing(src, dest_dir)
}

fn landing(src: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    let name = src.file_name().ok_or("That item has no name.")?;
    let target = dest_dir.join(name);
    if target.symlink_metadata().is_ok() {
        return Err(format!(
            "\u{201c}{}\u{201d} already exists in the target folder.",
            name.to_string_lossy()
        ));
    }
    Ok(target)
}

/// Where `path` is after `old` was renamed or moved to `new`; `None` when
/// `path` is neither `old` nor inside it.
pub fn rebase(path: &Path, old: &Path, new: &Path) -> Option<PathBuf> {
    path.strip_prefix(old).ok().map(|rest| new.join(rest))
}

pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
