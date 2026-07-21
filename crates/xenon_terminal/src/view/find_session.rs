//! Find session state and scrollback jump logic (cmd-f in the terminal).

use gpui::{App, Context, FocusHandle, Window};

use super::{State, TerminalView};
use crate::find::{self, FindOptions};

/// Session for the in-terminal find bar.
pub(super) struct FindSession {
    pub query: String,
    pub options: FindOptions,
    pub match_count: usize,
    pub current: Option<usize>,
    pub error: Option<String>,
    pub focus: FocusHandle,
    /// Bar is visible; when false, session still holds last query/options.
    pub open: bool,
    /// Bumped on each rescan so async results discard if stale.
    pub scan_token: u64,
}

impl FindSession {
    pub fn new(focus: FocusHandle) -> Self {
        Self {
            query: String::new(),
            options: FindOptions::default(),
            match_count: 0,
            current: None,
            error: None,
            focus,
            open: false,
            scan_token: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum FindToggle {
    Case,
    Word,
    Regex,
}

impl TerminalView {
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let seed = self.selection_seed(cx);
        let find = self
            .find
            .get_or_insert_with(|| FindSession::new(cx.focus_handle()));
        find.open = true;
        if let Some(seed) = seed {
            find.query = seed;
        }
        self.rescan_find(true, cx);
        if let Some(find) = &self.find {
            find.focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = &mut self.find {
            find.open = false;
            find.match_count = 0;
            find.current = None;
            find.error = None;
            find.scan_token = find.scan_token.wrapping_add(1);
        }
        self.clear_terminal_matches(cx);
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
        if self.find.as_ref().is_some_and(|f| f.match_count == 0) {
            // No cached matches (e.g. first ⌘G) — rescan then jump on completion.
            self.rescan_find(true, cx);
            return;
        }
        let Some(find) = &mut self.find else {
            return;
        };
        find.current = find::step_index(find.match_count, find.current, reverse);
        if let Some(idx) = find.current {
            self.activate_match(idx, cx);
        }
        cx.notify();
    }

    pub(super) fn append_find_query(&mut self, text: &str, cx: &mut Context<Self>) {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() {
            return;
        }
        if let Some(find) = &mut self.find {
            find.query.push_str(&clean);
        }
        self.rescan_find(true, cx);
        cx.notify();
    }

    pub(super) fn find_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(find) = &mut self.find {
            find.query.pop();
        }
        self.rescan_find(true, cx);
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
        self.rescan_find(true, cx);
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
        if find.match_count == 0 {
            return "No results".into();
        }
        let n = find.current.map(|i| i + 1).unwrap_or(1);
        format!("{n} of {}", find.match_count)
    }

    /// Re-run search over scrollback. `jump` selects a match after results land.
    pub(super) fn rescan_find(&mut self, jump: bool, cx: &mut Context<Self>) {
        let Some(find) = &self.find else {
            return;
        };
        if !find.open {
            return;
        }
        let query = find.query.clone();
        let options = find.options;
        let keep = find.current;
        let scan_token = {
            let find = self.find.as_mut().unwrap();
            find.scan_token = find.scan_token.wrapping_add(1);
            find.scan_token
        };

        if query.is_empty() {
            if let Some(find) = &mut self.find {
                find.match_count = 0;
                find.current = None;
                find.error = None;
            }
            self.clear_terminal_matches(cx);
            return;
        }

        let search = match find::build_search(&query, options) {
            Ok(Some(s)) => s,
            Ok(None) => {
                if let Some(find) = &mut self.find {
                    find.match_count = 0;
                    find.current = None;
                    find.error = None;
                }
                self.clear_terminal_matches(cx);
                return;
            }
            Err(err) => {
                if let Some(find) = &mut self.find {
                    find.match_count = 0;
                    find.current = None;
                    find.error = Some(err);
                }
                self.clear_terminal_matches(cx);
                return;
            }
        };

        let State::Ready(terminal) = &self.state else {
            return;
        };
        let terminal = terminal.clone();
        let task = terminal.update(cx, |term, cx| term.find_matches(search, cx));
        self._find_task = cx.spawn(async move |view, cx| {
            let matches = task.await;
            view.update(cx, |view, cx| {
                view.apply_find_matches(scan_token, matches, keep, jump, cx);
            })
            .ok();
        });
    }

    fn apply_find_matches(
        &mut self,
        scan_token: u64,
        matches: Vec<terminal::Range>,
        keep: Option<usize>,
        jump: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(find) = &mut self.find else {
            return;
        };
        if find.scan_token != scan_token || !find.open {
            return;
        }
        let count = matches.len();
        find.match_count = count;
        find.error = None;
        find.current = if count == 0 {
            None
        } else if let Some(i) = keep.filter(|&i| i < count) {
            Some(i)
        } else if jump {
            // Prefer the last match (closest to the live prompt / cursor).
            Some(count - 1)
        } else {
            Some(0)
        };
        let idx = find.current;

        if let State::Ready(terminal) = &self.state {
            terminal.update(cx, |term, _| {
                term.matches = matches;
            });
            if jump && let Some(idx) = idx {
                self.activate_match(idx, cx);
            }
        }
        cx.notify();
    }

    fn activate_match(&self, index: usize, cx: &mut Context<Self>) {
        let State::Ready(terminal) = &self.state else {
            return;
        };
        terminal.update(cx, |term, _| term.activate_match(index));
    }

    fn clear_terminal_matches(&self, cx: &mut Context<Self>) {
        if let State::Ready(terminal) = &self.state {
            terminal.update(cx, |term, _| term.matches.clear());
        }
    }

    fn selection_seed(&self, cx: &App) -> Option<String> {
        let State::Ready(terminal) = &self.state else {
            return None;
        };
        let text = terminal.read(cx).last_content().selection_text.clone()?;
        if text.is_empty() || text.contains('\n') {
            return None;
        }
        Some(text)
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
