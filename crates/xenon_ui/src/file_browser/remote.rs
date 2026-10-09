use super::{FileBrowser, TreeRow};
use std::path::{Path, PathBuf};

struct RemoteQuery<'a> {
    root: &'a Path,
    entries: &'a [xenon_ssh::RemoteEntry],
    open_file: Option<&'a Path>,
}

impl FileBrowser {
    pub(crate) fn install_remote(&mut self, root: PathBuf, entries: Vec<xenon_ssh::RemoteEntry>) {
        self.remote_entries.insert(root, entries);
    }

    pub(super) fn remote_rows(&self, root: &Path, open_file: Option<&Path>) -> Vec<TreeRow> {
        let Some(entries) = self.remote_entries.get(root) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        self.push_remote(
            &RemoteQuery {
                root,
                entries,
                open_file,
            },
            root,
            &mut rows,
        );
        rows
    }

    fn push_remote(&self, query: &RemoteQuery, dir: &Path, rows: &mut Vec<TreeRow>) {
        let RemoteQuery {
            root,
            entries,
            open_file,
        } = query;
        let mut children: Vec<_> = entries
            .iter()
            .filter(|entry| root.join(&entry.path).parent() == Some(dir))
            .collect();
        children.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
        });
        for entry in children {
            let path = root.join(&entry.path);
            let expanded = entry.is_dir && self.expanded_dirs.contains(&path);
            rows.push(TreeRow {
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                depth: Path::new(&entry.path)
                    .components()
                    .count()
                    .saturating_sub(1),
                is_open: *open_file == Some(path.as_path()),
                path: path.clone(),
                is_dir: entry.is_dir,
                is_symlink: entry.is_symlink,
                expanded,
            });
            if expanded {
                self.push_remote(query, &path, rows);
            }
        }
    }
}
