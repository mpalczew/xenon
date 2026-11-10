use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::Entry;

/// Skips what `.gitignore`, `.ignore` and git's exclude files list, plus
/// `.git` itself, which no ignore file names.
fn walker(root: &Path, hidden: bool, max_depth: Option<usize>) -> ignore::Walk {
    WalkBuilder::new(root)
        .hidden(hidden)
        .max_depth(max_depth)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
}

/// Every file and folder under `root`, as paths relative to it.
pub(crate) fn collect(root: &Path) -> Vec<Entry> {
    walker(root, false, None)
        .flatten()
        .filter_map(|entry| {
            let file_type = entry.file_type()?;
            let relative = entry.path().strip_prefix(root).ok()?;
            ((file_type.is_dir() || file_type.is_file()) && !relative.as_os_str().is_empty()).then(
                || Entry {
                    path: relative.to_string_lossy().into_owned(),
                    is_dir: file_type.is_dir(),
                },
            )
        })
        .collect()
}

/// Visible folders up to `max_depth` below `root`, shallowest first. Capped so
/// a huge tree cannot stall the caller.
pub(crate) fn subdirs(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut dirs: Vec<(usize, PathBuf)> = walker(root, true, Some(max_depth))
        .flatten()
        .filter(|e| e.depth() > 0 && e.file_type().is_some_and(|t| t.is_dir()))
        .take(5_000)
        .map(|e| (e.depth(), e.into_path()))
        .collect();
    dirs.sort();
    dirs.into_iter().map(|(_, path)| path).collect()
}
