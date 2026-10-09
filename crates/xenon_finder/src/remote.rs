use super::{Entry, FileIndex};

impl FileIndex {
    /// Index paths supplied by a remote workspace rather than a local walk.
    pub fn from_paths(paths: impl IntoIterator<Item = (String, bool)>) -> Self {
        Self {
            entries: paths
                .into_iter()
                .map(|(path, is_dir)| Entry { path, is_dir })
                .collect(),
        }
    }
}
