//! Find session state and buffer jump logic (cmd-f).

use std::ops::Range;

use gpui::{Context, FocusHandle, Window};

use super::{Content, EditorView};
use crate::find::{self, FindOptions, FindScan};

/// Session for the in-editor find bar (independent of vim `/`).
pub(super) struct FindSession {
    pub query: String,
    pub options: FindOptions,
    pub matches: Vec<Range<usize>>,
    pub current: Option<usize>,
    pub error: Option<String>,
    pub focus: FocusHandle,
    /// Bar is visible; when false, session still holds last query/options.
    pub open: bool,
}

impl FindSession {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            query: String::new(),
            options: FindOptions::default(),
            matches: Vec::new(),
            current: None,
            error: None,
            focus,
            open: false,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum FindToggle {
    Case,
    Word,
    Regex,
}

impl EditorView {
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.content, Content::Text(_)) {
            return;
        }
        let seed = match &self.content {
            Content::Text(buffer) => {
                let sel = buffer.selected_text();
                if !sel.is_empty() && !sel.contains('\n') {
                    Some(sel)
                } else {
                    None
                }
            }
            _ => None,
        };
        let find = self
            .find
            .get_or_insert_with(|| FindSession::new(cx.focus_handle()));
        find.open = true;
        if let Some(seed) = seed {
            find.query = seed;
        }
        self.rescan_find();
        self.jump_to_current_match();
        if let Some(find) = &self.find {
            find.focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = &mut self.find {
            find.open = false;
            find.matches.clear();
            find.current = None;
            find.error = None;
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub fn find_next(&mut self, cx: &mut Context<Self>) {
        self.step_find(false, cx);
    }

    pub fn find_previous(&mut self, cx: &mut Context<Self>) {
        self.step_find(true, cx);
    }

    fn step_find(&mut self, reverse: bool, cx: &mut Context<Self>) {
        if self.find.as_ref().is_none_or(|f| f.query.is_empty()) {
            return;
        }
        self.rescan_find();
        let Some(find) = &mut self.find else {
            return;
        };
        if find.matches.is_empty() {
            cx.notify();
            return;
        }
        find.current = find::step_index(&find.matches, find.current, reverse);
        self.jump_to_current_match();
        cx.notify();
    }

    pub(super) fn rescan_find(&mut self) {
        let Some(find) = &self.find else {
            return;
        };
        let query = find.query.clone();
        let options = find.options;
        let cursor = match &self.content {
            Content::Text(buffer) => buffer.cursor(),
            _ => 0,
        };
        let text = match &self.content {
            Content::Text(buffer) => buffer.text(),
            _ => return,
        };
        let FindScan { matches, error } = find::scan(&text, &query, options);
        let pick = find
            .current
            .filter(|&i| i < matches.len())
            .or_else(|| find::index_at_or_after(&matches, cursor));
        if let Some(find) = &mut self.find {
            find.matches = matches;
            find.error = error;
            find.current = pick;
        }
    }

    fn jump_to_current_match(&mut self) {
        let Some(find) = &self.find else {
            return;
        };
        let Some(idx) = find.current else {
            return;
        };
        let Some(range) = find.matches.get(idx).cloned() else {
            return;
        };
        let Content::Text(buffer) = &mut self.content else {
            return;
        };
        buffer.set_selection(range.end, range.start);
        self.last_cursor = None;
    }

    pub(super) fn append_find_query(&mut self, text: &str, cx: &mut Context<Self>) {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() {
            return;
        }
        if let Some(find) = &mut self.find {
            find.query.push_str(&clean);
        }
        self.rescan_find();
        self.jump_to_current_match();
        cx.notify();
    }

    pub(super) fn find_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(find) = &mut self.find {
            find.query.pop();
        }
        self.rescan_find();
        self.jump_to_current_match();
        cx.notify();
    }

    pub(super) fn toggle_find_option(&mut self, which: FindToggle, cx: &mut Context<Self>) {
        if let Some(find) = &mut self.find {
            match which {
                FindToggle::Case => find.options.case_sensitive = !find.options.case_sensitive,
                FindToggle::Word => find.options.whole_word = !find.options.whole_word,
                FindToggle::Regex => find.options.regex = !find.options.regex,
            }
        }
        self.rescan_find();
        self.jump_to_current_match();
        cx.notify();
    }

    pub(super) fn find_is_open(&self) -> bool {
        self.find.as_ref().is_some_and(|f| f.open)
    }

    pub(super) fn find_bar_focused(&self, window: &Window) -> bool {
        self.find
            .as_ref()
            .is_some_and(|f| f.open && f.focus.is_focused(window))
    }

    pub(super) fn find_status_label(&self) -> String {
        let Some(find) = &self.find else {
            return String::new();
        };
        if let Some(err) = &find.error {
            return truncate_err(err);
        }
        if find.query.is_empty() {
            return String::new();
        }
        if find.matches.is_empty() {
            return "No results".into();
        }
        let n = find.current.map(|i| i + 1).unwrap_or(1);
        format!("{n} of {}", find.matches.len())
    }
}

fn truncate_err(err: &str) -> String {
    const MAX: usize = 40;
    if err.len() <= MAX {
        format!("Invalid: {err}")
    } else {
        format!("Invalid: {}…", &err[..MAX])
    }
}
