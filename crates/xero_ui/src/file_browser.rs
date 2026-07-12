use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

pub(crate) fn dir_marker(is_dir: bool, expanded: bool) -> &'static str {
    match (is_dir, expanded) {
        (false, _) => "",
        (true, true) => "▾",
        (true, false) => "▸",
    }
}

pub(crate) fn file_icon(row: &TreeRow) -> &'static str {
    if row.is_dir {
        return "▭";
    }
    match row
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
    {
        "md" | "markdown" | "mdx" => "▰",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tif" | "tiff" => "▣",
        "rs" | "js" | "ts" | "tsx" | "py" | "toml" | "json" => "◆",
        "sh" | "bash" | "zsh" => "▸",
        "env" => "≡",
        _ if row.name == ".gitignore" => "⌁",
        _ => "·",
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
