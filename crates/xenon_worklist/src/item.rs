use anyhow::{Result, ensure};
use unicode_segmentation::UnicodeSegmentation;

pub const TITLE_LIMIT: usize = 80;

/// Length of a title as the list counts it, in graphemes.
pub fn title_length(title: &str) -> usize {
    title.graphemes(true).count()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OutlinePoint {
    text: String,
    depth: u8,
}

fn points_from_details(details: &str) -> Vec<OutlinePoint> {
    details
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let spaces = line
                .chars()
                .take_while(|character| *character == ' ')
                .count();
            let body = line[spaces..].trim();
            let text = body.strip_prefix("- ").unwrap_or(body).trim().to_owned();
            if text.is_empty() {
                return None;
            }
            Some(OutlinePoint {
                text,
                depth: u8::try_from(spaces / 2).unwrap_or(u8::MAX),
            })
        })
        .collect()
}

/// A validated item body: a one-line title, outline details, and task state
/// (`None` = a note, not a task).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemText {
    title: String,
    details: Vec<OutlinePoint>,
    done: Option<bool>,
}

impl ItemText {
    pub fn new(title: &str, details: &str, done: Option<bool>) -> Result<Self> {
        let title = title.trim();
        ensure!(!title.is_empty(), "Enter a title");
        ensure!(!title.contains(['\r', '\n']), "Title must be one line");
        let details = points_from_details(details);
        Ok(Self {
            title: title.to_owned(),
            details,
            done,
        })
    }

    /// An unchecked task with no details: the quick-add row and the phone.
    pub fn task(title: &str) -> Result<Self> {
        Self::new(title, "", Some(false))
    }

    pub(super) fn markdown(&self, newline: &str) -> String {
        let marker = match self.done {
            None => "- ",
            Some(true) => "- [x] ",
            Some(false) => "- [ ] ",
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
