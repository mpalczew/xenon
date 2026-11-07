//! Ctrl+click on terminal links: URLs, OSC 8 hyperlinks, and file paths.
//! Shift+click is left to VTE (its "bypass mouse reporting" modifier); plain
//! clicks stay with the shell or TUI.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::{gdk, graphene};
use vte::prelude::*;
use xenon_core::{TabId, WorkspaceId};

use crate::app::App;
use crate::term::TermView;

// PCRE2_MULTILINE: VTE requires it for match regexes.
const PCRE2_MULTILINE: u32 = 0x0000_0400;
const URL_RE: &str = r#"https?://[^\s<>"'`)]+"#;
const PATH_RE: &str = r"(?:\.{0,2}/)?[\w.\-+@~]+(?:/[\w.\-+@~]+)+(?::\d+)?";

/// What a click landed on.
pub(crate) enum Link {
    Url(String),
    Path(PathBuf),
}

/// Classify matched text. Relative paths resolve against `cwd`; `~/` against
/// home. A trailing `:line` is split off.
pub(crate) fn classify(text: &str, cwd: &Path) -> Link {
    if text.contains("://") {
        return Link::Url(text.to_string());
    }
    let raw = match text.rsplit_once(':') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => head,
        _ => text,
    };
    let path = if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var_os("HOME").map_or_else(|| PathBuf::from(raw), |h| PathBuf::from(h).join(rest))
    } else if raw.starts_with('/') {
        PathBuf::from(raw)
    } else {
        cwd.join(raw)
    };
    Link::Path(path)
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
    // Weak: the controller lives on `root`, so a strong capture would be a cycle
    // that keeps the terminal (and its shell) alive after the tab closes.
    let (weak_app, root, term) = (
        Rc::downgrade(app),
        view.root.downgrade(),
        view.term.downgrade(),
    );
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
            app.open_link(ws, tab, &text);
        }
    });
    view.root.add_controller(click);
}
