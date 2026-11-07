//! What a terminal paste of files or an image types: shell-quoted paths.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Shell-quoted path list for paste or drag-drop into the PTY.
pub fn terminal_paths_text(paths: impl IntoIterator<Item = impl AsRef<Path>>) -> String {
    paths
        .into_iter()
        .map(shell_path)
        .collect::<Vec<_>>()
        .join(" ")
}

/// One path, quoted for a POSIX shell only when it needs it.
pub fn shell_path(path: impl AsRef<Path>) -> String {
    let text = path.as_ref().to_string_lossy();
    if text.bytes().all(is_shell_safe) {
        return text.into_owned();
    }
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn is_shell_safe(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b':' | b'+' | b'=')
}

/// Where a pasted clipboard image is saved: a fresh PNG name under the temp
/// folder. The caller creates the folder and writes the file.
pub fn clipboard_image_path(id: u64) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir()
        .join("xenon-clipboard")
        .join(format!("clipboard-{nanos}-{id}.png"))
}
