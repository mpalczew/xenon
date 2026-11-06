//! Scripted capability demo (--selftest) and the offscreen PNG render path.

use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, glib};
use vte::prelude::*;

use crate::Ctx;
use crate::source::open_source;
use crate::term::{PCRE2_MULTILINE, current_terminal, dump_screen, new_terminal};

pub(crate) fn selftest_phase1(ctx: &Rc<Ctx>, win: &adw::ApplicationWindow, app: &adw::Application) {
    let Some(term) =
        current_terminal(ctx).or_else(|| ctx.stats.borrow().first().map(|x| x.1.clone()))
    else {
        return;
    };
    if !ctx.args.selftest {
        screenshot_and_quit(win, app, ctx.args.screenshot.as_deref());
        return;
    }
    log!("--- selftest start ---");
    // Output: OSC 8 link, URL, path, colors, title, bell.
    let script = concat!(
        "clear; printf '\\033]8;;http://osc8.example/\\033\\\\LINK\\033]8;;\\033\\\\ see https://example.org/x and /etc/hostname:3\\n';",
        "printf '\\033[31mred\\033[0m \\033[1;32mboldgreen\\033[0m \\033[48;2;10;20;30m\\033[38;2;250;200;100mtruecolor\\033[0m\\n';",
        "printf '\\033]0;spike-title\\a'; printf '\\a'\n"
    );
    term.feed_child(script.as_bytes());

    let ctx = ctx.clone();
    let win = win.clone();
    let app = app.clone();
    glib::timeout_add_local_once(Duration::from_millis(1500), move || {
        let term = term;
        let (cw, ch) = (term.char_width() as f64, term.char_height() as f64);
        // Row 0: "LINK see https://example.org/x and /etc/hostname:3"
        let at = |col: f64| (col * cw + cw / 2.0, ch / 2.0);
        let (x, y) = at(1.0);
        log!(
            "check_hyperlink_at(LINK) = {:?}",
            term.check_hyperlink_at(x, y)
        );
        let (x, y) = at(14.0);
        log!("check_match_at(url) = {:?}", term.check_match_at(x, y));
        let (x, y) = at(40.0);
        log!("check_match_at(path) = {:?}", term.check_match_at(x, y));
        let (x, y) = at(5.0);
        log!(
            "check_match_at(plain word 'see') = {:?}",
            term.check_match_at(x, y)
        );
        log!("window_title = {:?}", term.window_title());
        let (t, _) = term.termprop_string("xterm.title");
        log!("termprop xterm.title = {:?}", t);

        // Per-cell attributes.
        let html = dump_screen(&term);
        let has_color = html.contains("color:");
        let rgb = html.contains("rgb(250");
        log!("html has color styles: {has_color}; truecolor span present: {rgb}");
        let first: String = html.chars().take(600).collect();
        log!("html head: {first}");
        let txt = term.text_format(vte::Format::Text).unwrap_or_default();
        log!("plain text row0: {:?}", txt.lines().next().unwrap_or(""));

        // Search.
        match vte::Regex::for_search("example", PCRE2_MULTILINE) {
            Ok(r) => {
                term.search_set_regex(Some(&r), 0);
                term.search_set_wrap_around(true);
                log!("search next -> {}", term.search_find_next());
                log!("search next -> {}", term.search_find_next());
                log!("search prev -> {}", term.search_find_previous());
                log!(
                    "selection after search: {:?}",
                    term.text_selected(vte::Format::Text)
                );
            }
            Err(e) => log!("search regex: {e}"),
        }

        // Resize grid.
        term.set_size(100, 30);
        log!(
            "set_size(100,30) -> {}x{}",
            term.column_count(),
            term.row_count()
        );

        // Input bytes: type a command, then read the file side effect later.
        term.feed_child(b"echo fed-from-rust > /tmp/xenon-gtk-fed.txt\n");

        // Output burst counter.
        let n = ctx
            .stats
            .borrow()
            .iter()
            .find(|(_, t, _)| *t == term)
            .map(|(_, _, s)| (s.contents_changed.get(), s.bells.get()));
        log!("contents-changed count / bells so far: {n:?}");

        // Child exit status in a second tab.
        new_terminal(&ctx, Some("exit 7".into()));

        // Vim demo on the first FILE, if any.
        let ctx2 = ctx.clone();
        glib::timeout_add_local_once(Duration::from_millis(1500), move || {
            vim_demo(&ctx2);
            let ctx3 = ctx2.clone();
            let win = win;
            let app = app;
            glib::timeout_add_local_once(Duration::from_millis(500), move || {
                let fed = std::fs::read_to_string("/tmp/xenon-gtk-fed.txt").unwrap_or_default();
                log!("feed_child side effect file: {:?}", fed.trim());
                let _ = ctx3;
                screenshot_and_quit(&win, &app, ctx3.args.screenshot.as_deref());
            });
        });
    });
}

pub(crate) fn vim_demo(ctx: &Rc<Ctx>) {
    let Some(f) = ctx.args.files.first() else {
        log!("vim demo skipped (no FILE)");
        return;
    };
    open_source(ctx, f);
    let page = ctx.view.selected_page().unwrap();
    let scroll = page.child();
    let (vim, buffer): (sourceview::VimIMContext, sourceview::Buffer) = unsafe {
        (
            scroll
                .data::<sourceview::VimIMContext>("vim")
                .unwrap()
                .as_ref()
                .clone(),
            scroll
                .data::<sourceview::Buffer>("buffer")
                .unwrap()
                .as_ref()
                .clone(),
        )
    };
    let win = ctx.win.borrow().clone().unwrap();
    // Synthetic key presses through GtkIMContext.filter_key (public GTK4 API), so Vim mode
    // is exercised without compositor input.
    let seat = gdk::Display::default().unwrap().default_seat().unwrap();
    let kb = seat.keyboard().unwrap();
    let surface = win.surface().unwrap();
    let display = gdk::Display::default().unwrap();
    let press = |key: gdk::Key, state: gdk::ModifierType| {
        // filter_key wants a hardware keycode; map the keyval through the keymap.
        let Some(k) = display.map_keyval(key).and_then(|v| v.into_iter().next()) else {
            log!("no keycode for {key:?}");
            return false;
        };
        let handled = vim.filter_key(true, &surface, &kb, 0, k.keycode(), state, k.group());
        vim.filter_key(false, &surface, &kb, 0, k.keycode(), state, k.group());
        handled
    };
    let none = gdk::ModifierType::empty();
    let shift = gdk::ModifierType::SHIFT_MASK;
    let text = |b: &sourceview::Buffer| {
        let (x, y) = b.bounds();
        b.text(&x, &y, false).to_string()
    };
    log!("vim before: {:?}", text(&buffer));
    vim.connect_execute_command(|_, c| {
        log!("vim execute-command signal: {c:?}");
        false
    });
    press(gdk::Key::Escape, none); // normal mode
    press(gdk::Key::g, none);
    press(gdk::Key::g, none); // gg: top of buffer
    press(gdk::Key::i, none); // insert mode
    // In insert mode the IM context declines printable keys (false) so GtkTextView inserts
    // them itself through its normal key path; synthetic filter_key cannot reach that path.
    log!(
        "vim insert mode: filter_key('X') handled = {}",
        press(gdk::Key::X, shift)
    );
    press(gdk::Key::Escape, none);
    log!("vim after 'iXY<Esc>': {:?}", text(&buffer));
    press(gdk::Key::x, none); // normal mode x deletes char under cursor
    log!("vim after 'x': {:?}", text(&buffer));
    press(gdk::Key::v, none); // visual mode
    press(gdk::Key::l, none);
    press(gdk::Key::l, none);
    press(gdk::Key::d, none); // delete selection
    log!("vim after 'vlld': {:?}", text(&buffer));
    press(gdk::Key::colon, shift);
    press(gdk::Key::w, none);
    log!("vim command bar text: {:?}", vim.command_bar_text());
    press(gdk::Key::Return, none); // :w<CR> fires the write signal
    log!("vim buffer end: {:?}", text(&buffer));
}

pub(crate) fn screenshot_and_quit(
    win: &adw::ApplicationWindow,
    app: &adw::Application,
    png: Option<&str>,
) {
    if let Some(path) = png {
        match render_png(win, path) {
            Ok(()) => log!("screenshot written: {path}"),
            Err(e) => log!("screenshot failed: {e}"),
        }
    }
    app.quit();
}

/// Offscreen render: WidgetPaintable -> Snapshot -> render node -> GskRenderer texture -> PNG.
pub(crate) fn render_png(win: &adw::ApplicationWindow, path: &str) -> Result<(), String> {
    let (w, h) = (win.width(), win.height());
    let paintable = gtk::WidgetPaintable::new(Some(win));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, w as f64, h as f64);
    let node = snapshot.to_node().ok_or("empty render node")?;
    let renderer = win
        .native()
        .and_then(|n| n.renderer())
        .ok_or("no renderer")?;
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).map_err(|e| e.to_string())
}
