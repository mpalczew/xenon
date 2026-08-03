use std::collections::HashSet;
use std::path::{Path, PathBuf};

use lucide_icons::Icon;

pub(crate) struct FileBrowser {
    expanded_dirs: HashSet<PathBuf>,
    open: bool,
    /// Keyboard cursor index into the flattened `rows()` list.
    cursor: Option<usize>,
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::with_open(true)
    }
}

impl FileBrowser {
    pub fn with_open(open: bool) -> Self {
        Self {
            expanded_dirs: HashSet::new(),
            open,
            cursor: None,
        }
    }
}

struct RowQuery<'a> {
    dir: &'a Path,
    depth: usize,
    open_file: Option<&'a Path>,
}

pub(crate) struct TreeRow {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    /// True when the path itself is a symlink (not merely a path under one).
    pub is_symlink: bool,
    pub expanded: bool,
    pub is_open: bool,
    pub depth: usize,
}

impl FileBrowser {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn toggle_dir(&mut self, path: PathBuf) {
        if !self.expanded_dirs.insert(path.clone()) {
            self.expanded_dirs.remove(&path);
        }
    }

    pub fn reveal_dir(&mut self, root: &Path, dir: &Path) {
        let mut current = Some(dir);
        while let Some(path) = current {
            if path == root || !path.starts_with(root) {
                break;
            }
            self.expanded_dirs.insert(path.to_path_buf());
            current = path.parent();
        }
        self.open = true;
    }

    pub(crate) fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub(crate) fn ensure_cursor(&mut self) {
        if self.cursor.is_none() {
            self.cursor = Some(0);
        }
    }

    pub(crate) fn move_cursor(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.cursor = None;
            return;
        }
        let cur = self.cursor.unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, (len - 1) as isize) as usize;
        self.cursor = Some(next);
    }

    pub(crate) fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_dirs.contains(path)
    }

    /// Point the keyboard cursor at `path` within `rows` (no-op if missing).
    pub(crate) fn select_path_in(&mut self, rows: &[TreeRow], path: &Path) {
        if let Some(i) = rows.iter().position(|r| r.path == path) {
            self.cursor = Some(i);
        }
    }

    pub fn rows(&self, root: &Path, open_file: Option<&Path>) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        self.push_rows(
            RowQuery {
                dir: root,
                depth: 0,
                open_file,
            },
            &mut rows,
        );
        rows
    }

    fn push_rows(&self, query: RowQuery, rows: &mut Vec<TreeRow>) {
        let RowQuery {
            dir,
            depth,
            open_file,
        } = query;
        for (path, name, is_dir, is_symlink) in sorted_entries(dir) {
            let expanded = is_dir && self.expanded_dirs.contains(&path);
            rows.push(TreeRow {
                is_open: open_file == Some(path.as_path()),
                path: path.clone(),
                name,
                is_dir,
                is_symlink,
                expanded,
                depth,
            });
            if expanded {
                self.push_rows(
                    RowQuery {
                        dir: &path,
                        depth: depth + 1,
                        open_file,
                    },
                    rows,
                );
            }
        }
    }
}

/// Expand/collapse chevron for directories; files keep an empty slot for alignment.
pub(crate) fn dir_marker(is_dir: bool, expanded: bool) -> Option<Icon> {
    match (is_dir, expanded) {
        (false, _) => None,
        (true, true) => Some(Icon::ChevronDown),
        (true, false) => Some(Icon::ChevronRight),
    }
}

/// Lucide glyph for a tree row (matches sidebar/toolbar icon language).
pub(crate) fn file_icon(row: &TreeRow) -> Icon {
    if row.is_symlink {
        return if row.is_dir {
            Icon::FolderSymlink
        } else {
            Icon::FileSymlink
        };
    }
    if row.is_dir {
        return if row.expanded {
            Icon::FolderOpen
        } else {
            Icon::Folder
        };
    }
    let ext = row
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    match ext {
        "md" | "markdown" | "mdx" | "txt" | "rst" | "adoc" => Icon::FileText,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tif" | "tiff" => {
            Icon::FileImage
        }
        "sh" | "bash" | "zsh" | "fish" | "ps1" => Icon::FileTerminal,
        "rs" | "js" | "jsx" | "ts" | "tsx" | "mts" | "cts" | "py" | "go" | "c" | "h" | "cc"
        | "cpp" | "rb" | "java" | "lua" | "html" | "htm" | "css" | "scss" | "swift" | "scala"
        | "ex" | "exs" | "hs" | "php" | "zig" | "dart" | "cs" | "sol" | "nix" | "proto" | "ml"
        | "mli" | "r" | "elm" | "svelte" | "vue" | "kt" | "kts" | "glsl" | "cmake" | "toml"
        | "json" | "jsonc" | "yml" | "yaml" | "xml" | "scm" => Icon::FileCode,
        "env" | "pem" | "key" => Icon::FileKey,
        "lock" => Icon::FileLock,
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "7z" | "rar" => Icon::FileArchive,
        "csv" | "tsv" | "xlsx" | "xls" => Icon::FileSpreadsheet,
        _ if row.name == ".gitignore" || row.name == ".gitattributes" => Icon::GitBranch,
        _ => Icon::File,
    }
}

fn sorted_entries(dir: &Path) -> Vec<(PathBuf, String, bool, bool)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = read
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let ft = entry.file_type().ok();
            // DirEntry::file_type is the link itself, not the target.
            let is_symlink = ft.as_ref().is_some_and(|t| t.is_symlink());
            // Follow the link so symlink→dir still expands as a folder.
            let is_dir = if is_symlink {
                path.is_dir()
            } else {
                ft.is_some_and(|t| t.is_dir())
            };
            (path, name, is_dir, is_symlink)
        })
        .collect();
    entries.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn marks_file_and_dir_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        fs::write(root.join("plain.txt"), "hi").unwrap();
        fs::create_dir(root.join("real_dir")).unwrap();
        symlink(root.join("plain.txt"), root.join("link.txt")).unwrap();
        symlink(root.join("real_dir"), root.join("link_dir")).unwrap();

        let browser = FileBrowser::default();
        let rows = browser.rows(root, None);
        let by_name: std::collections::HashMap<_, _> =
            rows.into_iter().map(|r| (r.name.clone(), r)).collect();

        let plain = by_name.get("plain.txt").unwrap();
        assert!(!plain.is_symlink);
        assert!(!plain.is_dir);
        assert_eq!(char::from(file_icon(plain)), char::from(Icon::FileText));

        let link = by_name.get("link.txt").unwrap();
        assert!(link.is_symlink);
        assert!(!link.is_dir);
        assert_eq!(char::from(file_icon(link)), char::from(Icon::FileSymlink));

        let dir = by_name.get("real_dir").unwrap();
        assert!(!dir.is_symlink);
        assert!(dir.is_dir);
        assert_eq!(char::from(file_icon(dir)), char::from(Icon::Folder));

        let link_dir = by_name.get("link_dir").unwrap();
        assert!(link_dir.is_symlink);
        assert!(link_dir.is_dir);
        assert_eq!(
            char::from(file_icon(link_dir)),
            char::from(Icon::FolderSymlink)
        );
    }
}
