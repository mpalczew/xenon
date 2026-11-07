//! Text a Git row shows for one changed file.

use crate::status::GitFile;

impl GitFile {
    /// Muted text after the name in a Git row: the rename source, else the
    /// deepest directories.
    pub fn detail(&self) -> String {
        match &self.from {
            Some(from) => format!("\u{2190} {from}"),
            None => self.dir_tail(),
        }
    }

    /// Hover text: the repo-relative path (with its rename source) and the change size.
    pub fn tooltip(&self) -> String {
        match &self.from {
            Some(from) => format!(
                "{from} \u{2192} {}  +{} -{}",
                self.path, self.added, self.removed
            ),
            None => format!("{}  +{} -{}", self.path, self.added, self.removed),
        }
    }

    /// Hover text: the full path under `root` (with its rename source) and
    /// the change size.
    pub fn full_tooltip(&self, root: &std::path::Path) -> String {
        let full = root.join(&self.path).display().to_string();
        match &self.from {
            Some(from) => format!(
                "{} \u{2192} {full}  +{} -{}",
                root.join(from).display(),
                self.added,
                self.removed
            ),
            None => format!("{full}  +{} -{}", self.added, self.removed),
        }
    }
}
