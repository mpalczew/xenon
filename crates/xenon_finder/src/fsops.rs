//! Creating and copying files for the Files tree.

use std::io;
use std::path::Path;

/// Copy a file, or a folder with everything in it. Links are copied as links.
pub fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    let kind = src.symlink_metadata()?.file_type();
    if kind.is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(src)?, dst)
    } else if kind.is_dir() {
        std::fs::create_dir(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dst.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst).map(drop)
    }
}

/// Create an empty file or folder; it must not exist yet.
pub fn create(path: &Path, is_dir: bool) -> io::Result<()> {
    if is_dir {
        std::fs::create_dir(path)
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(drop)
    }
}
