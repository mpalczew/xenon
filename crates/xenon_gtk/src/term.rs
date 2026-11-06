//! VTE terminal tabs: setup, matches, hyperlinks, signals, screen dump.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib, graphene};
use vte::prelude::*;

use crate::{Ctx, Stats};

// PCRE2_MULTILINE: VTE requires it for match and search regexes.
pub(crate) const PCRE2_MULTILINE: u32 = 0x0000_0400;
const URL_RE: &str = r#"https?://[^\s<>"'`)]+"#;
const PATH_RE: &str = r"(?:\.{0,2}/)?[\w.\-+@~]+(?:/[\w.\-+@~]+)+(?::\d+)?";

pub(crate) fn current_terminal(ctx: &Ctx) -> Option<vte::Terminal> {
    let page = ctx.view.selected_page()?;
    ctx.stats
        .borrow()
        .iter()
        .find(|(p, _, _)| *p == page)
        .map(|(_, t, _)| t.clone())
}

pub(crate) fn new_terminal(ctx: &Rc<Ctx>, cmd: Option<String>) -> vte::Terminal {
    let id = ctx.next_id.get();
    ctx.next_id.set(id + 1);
    let term = vte::Terminal::new();
    term.set_hexpand(true);
    term.set_vexpand(true);
    term.set_scrollback_lines(10_000);
    term.set_font_desc(Some(&gtk::pango::FontDescription::from_string(
        "Monospace 11",
    )));
    let fg = gdk::RGBA::parse("#d8dee9").unwrap();
    let bg = gdk::RGBA::parse("#1e222a").unwrap();
    term.set_colors(Some(&fg), Some(&bg), &[]);

    // OSC 8 hyperlinks.
    term.set_allow_hyperlink(true);

    // Regex matches for URLs and file paths. Tags are kept for the click handler.
    for pat in [URL_RE, PATH_RE] {
        match vte::Regex::for_match(pat, PCRE2_MULTILINE) {
            Ok(re) => {
                let tag = term.match_add_regex(&re, 0);
                term.match_set_cursor_name(tag, "pointer");
            }
            Err(e) => log!("regex compile failed: {e}"),
        }
    }

    let stats = Rc::new(Stats::default());
    {
        let s = stats.clone();
        term.connect_bell(move |_| {
            s.bells.set(s.bells.get() + 1);
            log!("bell #{}", s.bells.get());
        });
    }
    {
        let s = stats.clone();
        term.connect_contents_changed(move |_| {
            // Counted, not logged: this fires on every output burst.
            s.contents_changed.set(s.contents_changed.get() + 1);
        });
    }
    // Wrapper so a capture-phase click gesture can sit above VTE's own gestures.
    let wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrap.append(&term);
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let term = term.clone();
        let wrap = wrap.clone();
        click.connect_pressed(move |g, _, x, y| {
            let state = g.current_event_state();
            // Ctrl+click (without Shift) opens links. Claiming the sequence keeps it from
            // reaching a TUI that has mouse reporting on. Shift+click is left to VTE,
            // which uses Shift as its "bypass mouse reporting" modifier.
            if !state.contains(gdk::ModifierType::CONTROL_MASK)
                || state.contains(gdk::ModifierType::SHIFT_MASK)
            {
                return;
            }
            let Some(p) = wrap.compute_point(&term, &graphene::Point::new(x as f32, y as f32))
            else {
                return;
            };
            let (px, py) = (p.x() as f64, p.y() as f64);
            if let Some(uri) = term.check_hyperlink_at(px, py) {
                log!("ctrl-click OSC8 hyperlink: {uri}");
                g.set_state(gtk::EventSequenceState::Claimed);
            } else if let (Some(text), tag) = term.check_match_at(px, py) {
                log!("ctrl-click regex match (tag {tag}): {text}");
                g.set_state(gtk::EventSequenceState::Claimed);
            }
        });
    }
    wrap.add_controller(click);

    let page = ctx.view.append(&wrap);
    page.set_title(&format!("term {id}"));
    ctx.stats
        .borrow_mut()
        .push((page.clone(), term.clone(), stats.clone()));

    {
        let page = page.clone();
        // VTE 0.84 title path: the "xterm.title" termprop (detail-filtered signal).
        let page2 = page.clone();
        term.connect_termprop_changed(Some("xterm.title"), move |t, name| {
            let (title, _) = t.termprop_string(name);
            let title = title.map(|s| s.to_string()).unwrap_or_default();
            log!("termprop-changed {name} = {title:?}");
            page2.set_title(&title);
        });
        // Legacy path (deprecated since 0.78, still emitted).
        term.connect_window_title_changed(move |t| {
            log!("legacy window-title-changed: {:?}", t.window_title());
        });
        let _ = &page;
    }
    {
        let view = ctx.view.clone();
        let page = page.clone();
        term.connect_child_exited(move |_, status| {
            // `status` is a raw wait status; exit code is bits 8..16.
            log!(
                "child-exited: raw status {status} (exit code {})",
                (status >> 8) & 0xff
            );
            view.close_page(&page);
        });
    }
    if let Some(re) = &ctx.args.search
        && let Ok(r) = vte::Regex::for_search(re, PCRE2_MULTILINE)
    {
        term.search_set_regex(Some(&r), 0);
        term.search_set_wrap_around(true);
    }
    ctx.view.set_selected_page(&page);
    spawn_shell(&term, cmd);
    term
}

pub(crate) fn spawn_shell(term: &vte::Terminal, cmd: Option<String>) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.display().to_string());
    // An empty envv would mean an empty environment, so pass ours explicitly.
    let mut env: Vec<String> = std::env::vars()
        .filter(|(k, _)| k != "TERM" && k != "COLORTERM")
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    env.push("TERM=xterm-256color".into());
    env.push("COLORTERM=truecolor".into());
    let env_refs: Vec<&str> = env.iter().map(String::as_str).collect();
    let t = term.clone();
    let shell_name = shell.clone();
    term.spawn_async(
        vte::PtyFlags::DEFAULT,
        cwd.as_deref(),
        &[&shell],
        &env_refs,
        glib::SpawnFlags::SEARCH_PATH,
        || {},
        -1,
        None::<&gio::Cancellable>,
        move |res| match res {
            Ok(pid) => {
                log!("spawned {shell_name} pid {}", pid.0);
                if let Some(c) = cmd {
                    t.feed_child(format!("{c}\n").as_bytes());
                }
            }
            Err(e) => log!("spawn failed: {e}"),
        },
    );
}

/// Screen text with per-cell attributes. VTE 0.84 offers Format::Text (plain) and
/// Format::Html (<font color>, <b>, background spans). text_format(Html) is the visible
/// screen; text_range_format(Html, row0..) works too but, when tested here, returned 5
/// extra leading blank rows.
pub(crate) fn dump_screen(t: &vte::Terminal) -> String {
    let rows = t.row_count();
    let cols = t.column_count();
    // Row numbers are absolute (scrollback included); the visible screen starts at the
    // vertical adjustment's value.
    let top = t.vadjustment().map(|a| a.value() as i64).unwrap_or(0);
    let (html, _) = t.text_range_format(vte::Format::Html, top, 0, top + rows - 1, cols);
    let html = html.map(|s| s.to_string()).unwrap_or_default();
    let vis = t
        .text_format(vte::Format::Html)
        .map(|s| s.to_string())
        .unwrap_or_default();
    log!(
        "text_format(Html) leading newlines after <pre>: {}, has color: {}",
        vis.trim_start_matches("<pre>")
            .chars()
            .take_while(|c| *c == '\n')
            .count(),
        vis.contains("<font")
    );
    let (rt, _) = t.text_range_format(vte::Format::Text, top, 0, top + rows - 1, cols);
    log!(
        "range-text leading newlines: {}",
        rt.map(|s| s.chars().take_while(|c| *c == '\n').count())
            .unwrap_or(0)
    );
    // Prefer the whole-screen call: its rows line up with the visible grid.
    let html = if vis.is_empty() { html } else { vis };
    let plain = t
        .text_format(vte::Format::Text)
        .map(|s| s.to_string())
        .unwrap_or_default();
    let _ = std::fs::write("/tmp/xenon-gtk-dump.html", &html);
    let _ = std::fs::write("/tmp/xenon-gtk-dump.txt", &plain);
    log!(
        "dump-screen: top row {top} (adj lower {:?}), {cols}x{rows}, html {} bytes, text {} bytes (/tmp/xenon-gtk-dump.*)",
        t.vadjustment().map(|a| (a.lower(), a.upper(), a.value())),
        html.len(),
        plain.len(),
    );
    html
}
