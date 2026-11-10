use adw::prelude::*;
use gtk::glib;
use vte::prelude::*;

// PCRE2 flags VTE accepts: it requires MULTILINE; CASELESS is optional.
const PCRE2_MULTILINE: u32 = 0x0000_0400;
const PCRE2_CASELESS: u32 = 0x0000_0008;

macro_rules! weak_term_fn {
    ($weak:expr, |$t:ident| $body:expr) => {{
        let weak = $weak.clone();
        move |_: &_| {
            if let Some($t) = weak.upgrade() {
                $body;
            }
        }
    }};
}
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
        let key = |accel| crate::keys::label(accel);
        prev.set_tooltip_text(Some(&format!(
            "Previous match \u{b7} {} / {}",
            key("<Shift>Return"),
            crate::keys::action_label("app.find-prev").unwrap_or_default(),
        )));
        next.set_tooltip_text(Some(&format!(
            "Next match \u{b7} {} / {}",
            key("Return"),
            crate::keys::action_label("app.find-next").unwrap_or_default(),
        )));
        close.set_tooltip_text(Some(&format!("Close \u{b7} {}", key("Escape"))));
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

        let weak_term = term.downgrade();
        entry.connect_search_changed({
            let weak = weak_term.clone();
            move |e| {
                if let Some(t) = weak.upgrade() {
                    set_query(&t, &e.text());
                }
            }
        });
        entry.connect_activate(weak_term_fn!(weak_term, |t| t.search_find_next()));
        entry.connect_next_match(weak_term_fn!(weak_term, |t| t.search_find_next()));
        entry.connect_previous_match(weak_term_fn!(weak_term, |t| t.search_find_previous()));
        prev.connect_clicked(weak_term_fn!(weak_term, |t| t.search_find_previous()));
        next.connect_clicked(weak_term_fn!(weak_term, |t| t.search_find_next()));
        let bar = Self { revealer, entry };
        let weak_revealer = bar.revealer.downgrade();
        close.connect_clicked(hide_fn(&weak_revealer, &weak_term));
        bar.entry
            .connect_stop_search(hide_fn(&weak_revealer, &weak_term));
        bar
    }

    pub(crate) fn open(&self) {
        self.revealer.set_reveal_child(true);
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
    }
}

fn hide_fn<T>(
    revealer: &glib::WeakRef<gtk::Revealer>,
    term: &glib::WeakRef<vte::Terminal>,
) -> impl Fn(&T) + 'static {
    let (revealer, term) = (revealer.clone(), term.clone());
    move |_| {
        let (Some(revealer), Some(term)) = (revealer.upgrade(), term.upgrade()) else {
            return;
        };
        revealer.set_reveal_child(false);
        term.search_set_regex(None, 0);
        glib::idle_add_local_once(move || {
            term.grab_focus();
        });
    }
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
