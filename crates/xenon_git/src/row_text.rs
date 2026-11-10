use crate::status::GitFile;

impl GitFile {
    pub fn detail(&self) -> String {
        match &self.from {
            Some(from) => format!("\u{2190} {from}"),
            None => self.dir_tail(),
        }
    }

    pub fn tooltip(&self) -> String {
        match &self.from {
            Some(from) => format!(
                "{from} \u{2192} {}  +{} -{}",
                self.path, self.added, self.removed
            ),
            None => format!("{}  +{} -{}", self.path, self.added, self.removed),
        }
    }

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
