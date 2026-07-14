use std::collections::HashSet;
use std::path::{Path, PathBuf};

use lucide_icons::Icon;

#[derive(Default)]
pub(crate) struct FileBrowser {
    expanded_dirs: HashSet<PathBuf>,
    open: bool,
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
        for (path, name, is_dir) in sorted_entries(dir) {
            let expanded = is_dir && self.expanded_dirs.contains(&path);
            rows.push(TreeRow {
                is_open: open_file == Some(path.as_path()),
                path: path.clone(),
                name,
                is_dir,
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

fn sorted_entries(dir: &Path) -> Vec<(PathBuf, String, bool)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = read
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry
                .file_type()
                .map(|entry| entry.is_dir())
                .unwrap_or(false);
            (entry.path(), name, is_dir)
        })
        .collect();
    entries.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    entries
}
