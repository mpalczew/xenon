//! The terminal find strip: a search entry above the terminal that drives VTE's
//! own search (match painting, wrap-around, scrolling to the match).

use adw::prelude::*;
use gtk::glib;
use vte::prelude::*;

// PCRE2 flags VTE accepts: it requires MULTILINE; CASELESS is optional.
const PCRE2_MULTILINE: u32 = 0x0000_0400;
const PCRE2_CASELESS: u32 = 0x0000_0008;

/// Search text as a literal PCRE2 pattern, plus whether to ignore case
/// (smartcase: only when the query has no capital letters).
pub(crate) fn search_pattern(query: &str) -> Option<(String, bool)> {
    if query.is_empty() {
        return None;
    }
    let mut pattern = String::new();
    for c in query.chars() {
        if !c.is_alphanumeric() && c != ' ' && c.is_ascii() {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    Some((pattern, !query.chars().any(char::is_uppercase)))
}

pub(crate) struct FindBar {
    pub revealer: gtk::Revealer,
    pub entry: gtk::SearchEntry,
}

impl FindBar {
    pub(crate) fn new(term: &vte::Terminal) -> Self {
        let entry = gtk::SearchEntry::new();
        entry.set_hexpand(true);
        entry.set_placeholder_text(Some("Find in terminal"));
        let prev = gtk::Button::from_icon_name("go-up-symbolic");
        let next = gtk::Button::from_icon_name("go-down-symbolic");
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        for b in [&prev, &next, &close] {
            b.add_css_class("flat");
            b.set_focusable(false);
        }
        prev.set_tooltip_text(Some("Previous (Shift+F3)"));
        next.set_tooltip_text(Some("Next (F3)"));
        close.set_tooltip_text(Some("Close (Escape)"));
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        row.add_css_class("xfind");
        row.append(&entry);
        row.append(&prev);
        row.append(&next);
        row.append(&close);
        let revealer = gtk::Revealer::new();
        revealer.set_child(Some(&row));
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(120);

        let t = term.clone();
        entry.connect_search_changed(move |e| set_query(&t, &e.text()));
        let t = term.clone();
        entry.connect_activate(move |_| {
            t.search_find_next();
        });
        let t = term.clone();
        entry.connect_next_match(move |_| {
            t.search_find_next();
        });
        let t = term.clone();
        entry.connect_previous_match(move |_| {
            t.search_find_previous();
        });
        let t = term.clone();
        prev.connect_clicked(move |_| {
            t.search_find_previous();
        });
        let t = term.clone();
        next.connect_clicked(move |_| {
            t.search_find_next();
        });
        let bar = Self { revealer, entry };
        let (r, t) = (bar.revealer.clone(), term.clone());
        close.connect_clicked(move |_| hide(&r, &t));
        let (r, t) = (bar.revealer.clone(), term.clone());
        bar.entry.connect_stop_search(move |_| hide(&r, &t));
        bar
    }

    /// Reveal the strip and put the cursor in the entry.
    pub(crate) fn open(&self) {
        self.revealer.set_reveal_child(true);
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
    }
}

fn hide(revealer: &gtk::Revealer, term: &vte::Terminal) {
    revealer.set_reveal_child(false);
    term.search_set_regex(None, 0);
    let term = term.clone();
    glib::idle_add_local_once(move || {
        term.grab_focus();
    });
}

fn set_query(term: &vte::Terminal, query: &str) {
    let Some((pattern, caseless)) = search_pattern(query) else {
        term.search_set_regex(None, 0);
        return;
    };
    let flags = PCRE2_MULTILINE | if caseless { PCRE2_CASELESS } else { 0 };
    match vte::Regex::for_search(&pattern, flags) {
        Ok(regex) => {
            term.search_set_regex(Some(&regex), 0);
            term.search_set_wrap_around(true);
            term.search_find_next();
        }
        Err(_) => term.search_set_regex(None, 0),
    }
}
