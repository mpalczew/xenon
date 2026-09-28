//! Validated worklist item and Markdown format.

use anyhow::{Result, ensure};
use unicode_segmentation::UnicodeSegmentation;
use xenon_design_system::{OutlinePoint, points_from_details};

/// A soft limit: longer titles save, and the editor points out the overflow.
pub const TITLE_LIMIT: usize = 80;

pub fn title_length(title: &str) -> usize {
    title.graphemes(true).count()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkItem {
    title: String,
    details: Vec<OutlinePoint>,
    task: bool,
}

impl WorkItem {
    pub fn new(title: &str, details: &str, task: bool) -> Result<Self> {
        let title = title.trim();
        ensure!(!title.is_empty(), "Enter a title");
        ensure!(!title.contains(['\r', '\n']), "Title must be one line");
        let details = points_from_details(details);
        Ok(Self {
            title: title.to_owned(),
            details,
            task,
        })
    }

    pub fn markdown(&self, checked: bool, newline: &str) -> String {
        let marker = if !self.task {
            "- "
        } else if checked {
            "- [x] "
        } else {
            "- [ ] "
        };
        let mut out = format!("{marker}{}", self.title);
        for detail in &self.details {
            let indent = 2 + usize::from(detail.depth) * 2;
            out.push_str(newline);
            out.push_str(&" ".repeat(indent));
            out.push_str("- ");
            out.push_str(&detail.text);
        }
        out
    }
}
