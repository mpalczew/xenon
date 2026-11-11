use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// Ignore files of one folder, strongest first: `.ignore`, then `.gitignore`.
type Folder = Arc<[Gitignore; 2]>;

/// Answers "would the finder's walker skip this path?" for one workspace root,
/// using the same rules as `walk.rs`: nested `.gitignore` and `.ignore` files,
/// git's `info/exclude` and global excludes, and `.git` itself. Dotfiles are
/// not hidden. Folder rules load on first use, so build and share it from a
/// worker thread.
pub struct IgnoreMatcher {
    root: PathBuf,
    /// Weakest rules: `info/exclude`, then the global excludes.
    base: [Gitignore; 2],
    folders: Mutex<HashMap<PathBuf, Folder>>,
}

impl IgnoreMatcher {
    pub fn new(root: &Path) -> IgnoreMatcher {
        let mut builder = GitignoreBuilder::new(root);
        builder.add(root.join(".git/info/exclude"));
        let exclude = builder.build().unwrap_or_else(|_| Gitignore::empty());
        IgnoreMatcher {
            root: root.to_path_buf(),
            base: [exclude, Gitignore::global().0],
            folders: Mutex::default(),
        }
    }

    /// True when `path` (absolute, under the root) is hidden from the finder.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        if path.file_name().is_some_and(|n| n == ".git") {
            return true;
        }
        let Some(parent) = path.parent().filter(|p| p.starts_with(&self.root)) else {
            return false;
        };
        let mut dirs: Vec<&Path> = parent
            .ancestors()
            .take_while(|d| d.starts_with(&self.root))
            .collect();
        dirs.reverse();
        let folders: Vec<Folder> = dirs.iter().map(|d| self.folder(d)).collect();
        folders
            .iter()
            .rev()
            .flat_map(|f| f.iter())
            .chain(&self.base)
            .map(|g| g.matched(path, is_dir))
            .find(|m| !m.is_none())
            .is_some_and(|m| m.is_ignore())
    }

    fn folder(&self, dir: &Path) -> Folder {
        let mut folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
        folders
            .entry(dir.to_path_buf())
            .or_insert_with(|| {
                let load = |name: &str| Gitignore::new(dir.join(name)).0;
                Arc::new([load(".ignore"), load(".gitignore")])
            })
            .clone()
    }
}
