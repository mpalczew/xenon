use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::{gdk, gio, glib, graphene};
use vte::prelude::*;
use xenon_core::{TabId, WorkspaceId};

use crate::app::App;
use crate::term::TermView;

// PCRE2_MULTILINE: VTE requires it for match regexes.
const PCRE2_MULTILINE: u32 = 0x0000_0400;
const URL_RE: &str = r#"https?://[^\s<>"'`)]+"#;
const PATH_RE: &str = r"(?:(?:\.{0,2}/)?[\w.\-+@~]+(?:/[\w.\-+@~]+)+|[\w\-+@][\w.\-+@]*\.[A-Za-z][A-Za-z0-9]{0,7})(?::\d+){0,2}";

pub(crate) enum Link {
    Url(String),
    Path(PathBuf, Option<u32>),
}

pub(crate) enum Opening {
    Editor(PathBuf, u32),
    Reveal(PathBuf),
    Launch(String),
    Default(PathBuf),
    Missing(PathBuf),
    Search(String, u32),
}

fn split_line(text: &str) -> (&str, Option<u32>) {
    let (mut raw, mut numbers) = (text, Vec::new());
    while let Some((head, tail)) = raw.rsplit_once(':') {
        match tail.parse::<u32>() {
            Ok(n) if numbers.len() < 2 => numbers.push(n),
            _ => break,
        }
        raw = head;
    }
    (raw, numbers.last().copied())
}

pub(crate) fn classify(text: &str, cwd: &Path) -> Link {
    if text.starts_with("file://")
        && let Some(path) = gio::File::for_uri(text).path()
    {
        return Link::Path(path, None);
    }
    if text.contains("://") {
        return Link::Url(text.to_string());
    }
    let (raw, line) = split_line(text);
    let path = if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var_os("HOME").map_or_else(|| PathBuf::from(raw), |h| PathBuf::from(h).join(rest))
    } else if raw.starts_with('/') {
        PathBuf::from(raw)
    } else {
        cwd.join(raw)
    };
    Link::Path(path, line)
}

pub(crate) fn opening_for(path: PathBuf, line: Option<u32>) -> Opening {
    if !path.exists() {
        Opening::Missing(path)
    } else if path.is_dir() {
        Opening::Reveal(path)
    } else if crate::image::is_image(&path) || crate::editor::check(&path).is_ok() {
        Opening::Editor(path, line.unwrap_or(1))
    } else {
        Opening::Default(path)
    }
}

pub(crate) fn decide(text: &str, cwd: &Path, root: &Path) -> Opening {
    match classify(text, cwd) {
        Link::Url(url) => Opening::Launch(url),
        Link::Path(path, line) if path.exists() => opening_for(path, line),
        Link::Path(path, line) => {
            let (raw, _) = split_line(text);
            if text.starts_with("file://") || raw.starts_with('/') || raw.starts_with("~/") {
                return Opening::Missing(path);
            }
            let in_root = root.join(raw);
            if in_root.exists() {
                opening_for(in_root, line)
            } else {
                Opening::Search(raw.to_string(), line.unwrap_or(1))
            }
        }
    }
}

pub(crate) fn install(app: &Rc<App>, view: &Rc<TermView>, ws: WorkspaceId, tab: TabId) {
    let term = &view.term;
    for pattern in [URL_RE, PATH_RE] {
        if let Ok(regex) = vte::Regex::for_match(pattern, PCRE2_MULTILINE) {
            let tag = term.match_add_regex(&regex, 0);
            term.match_set_cursor_name(tag, "pointer");
        }
    }
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let (weak_app, root, term) = (
        Rc::downgrade(app),
        view.root.downgrade(),
        view.term.downgrade(),
    );
    // sync: event-time read: reads the terminal under the pointer and claims the gesture now; opening the link is deferred.
    click.connect_pressed(move |gesture, _, x, y| {
        let state = gesture.current_event_state();
        if !state.contains(gdk::ModifierType::CONTROL_MASK)
            || state.contains(gdk::ModifierType::SHIFT_MASK)
        {
            return;
        }
        let (Some(root), Some(term)) = (root.upgrade(), term.upgrade()) else {
            return;
        };
        let Some(p) = root.compute_point(&term, &graphene::Point::new(x as f32, y as f32)) else {
            return;
        };
        let (px, py) = (f64::from(p.x()), f64::from(p.y()));
        let text = term
            .check_hyperlink_at(px, py)
            .map(|s| s.to_string())
            .or_else(|| term.check_match_at(px, py).0.map(|s| s.to_string()));
        if let (Some(text), Some(app)) = (text, weak_app.upgrade()) {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            glib::idle_add_local_once(move || app.open_link(ws, tab, &text));
        }
    });
    view.root.add_controller(click);
}
