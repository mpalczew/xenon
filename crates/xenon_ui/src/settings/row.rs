//! One setting as data. Rendering, keyboard activation, and search all read
//! the same rows, so a setting cannot be visible but unreachable by keys.

use std::rc::Rc;

use gpui::{App, Hsla, SharedString, Window};
use theme::Appearance;

use super::page::SettingsPage;
use crate::dropdown::{DropdownId, SizeTarget};

pub(super) type Run = Rc<dyn Fn(&mut Window, &mut App)>;
pub(super) type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// How the line under a setting's name reads.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Muted,
    Good,
    Mono,
}

/// Inline text fields Settings can edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    RemoteHostname,
    RustServer,
    TypeScriptServer,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionKind {
    Primary,
    Secondary,
    Quiet,
    Destructive,
}

#[derive(Clone)]
pub(super) struct RowAction {
    pub label: SharedString,
    pub kind: ActionKind,
    pub run: Run,
}

#[derive(Clone)]
pub(super) struct Swatch {
    pub name: SharedString,
    pub colors: [Hsla; 3],
    pub current: bool,
    pub appearance: Appearance,
}

#[derive(Clone)]
pub(super) enum Control {
    None,
    Switch {
        on: bool,
        toggle: Run,
    },
    Choice {
        options: Vec<SharedString>,
        selected: usize,
        pick: Pick,
    },
    Font {
        family: DropdownId,
        size: SizeTarget,
    },
    Swatches(Vec<Swatch>),
    Field {
        field: Field,
        /// Stored text; empty means automatic.
        value: String,
        shown: SharedString,
    },
    Action(RowAction),
    Value(SharedString),
}

#[derive(Clone)]
pub(super) struct SettingRow {
    pub id: SharedString,
    pub label: SharedString,
    pub detail: Option<SharedString>,
    pub tone: Tone,
    pub keywords: &'static str,
    pub control: Control,
}

impl SettingRow {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        control: Control,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            tone: Tone::Muted,
            keywords: "",
            control,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn keywords(mut self, keywords: &'static str) -> Self {
        self.keywords = keywords;
        self
    }

    /// Case-insensitive match on name, detail, and extra keywords.
    pub fn matches(&self, needle: &str) -> bool {
        let haystack = format!(
            "{} {} {}",
            self.label,
            self.detail.as_deref().unwrap_or_default(),
            self.keywords
        );
        haystack.to_lowercase().contains(&needle.to_lowercase())
    }
}

pub(super) struct Group {
    pub title: Option<SharedString>,
    pub rows: Vec<SettingRow>,
}

impl Group {
    pub fn untitled(rows: Vec<SettingRow>) -> Self {
        Self { title: None, rows }
    }
}

/// One keyboard-reachable row and where it lives.
pub(super) struct Located {
    pub page: SettingsPage,
    pub index: usize,
    pub row: SettingRow,
}

pub(super) fn page_groups(page: SettingsPage, cx: &App) -> Vec<Group> {
    match page {
        SettingsPage::Appearance => super::appearance::groups(cx),
        SettingsPage::Fonts => super::pages::fonts(),
        SettingsPage::Editor => super::pages::editor(cx),
        SettingsPage::Terminal => super::pages::terminal(cx),
        SettingsPage::Agents => super::agents::groups(),
        SettingsPage::PhoneRemote => super::remote_page::groups(cx),
        SettingsPage::LanguageServers => super::pages::language_servers(),
    }
}

/// Rows in keyboard order: one page, or every match when searching.
pub(super) fn visible_rows(page: SettingsPage, query: &str, cx: &App) -> Vec<Located> {
    let pages: &[SettingsPage] = if query.is_empty() {
        std::slice::from_ref(&page)
    } else {
        &SettingsPage::ALL
    };
    pages
        .iter()
        .flat_map(|&page| {
            page_groups(page, cx)
                .into_iter()
                .flat_map(|group| group.rows)
                .enumerate()
                .map(move |(index, row)| Located { page, index, row })
        })
        .filter(|located| query.is_empty() || located.row.matches(query))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_name_detail_and_keywords() {
        let row = SettingRow::new("wrap", "Wrap prose", Control::None)
            .detail("Markdown and plain text")
            .keywords("soft");
        assert!(row.matches("WRAP"));
        assert!(row.matches("markdown"));
        assert!(row.matches("soft"));
        assert!(!row.matches("vim"));
    }
}
