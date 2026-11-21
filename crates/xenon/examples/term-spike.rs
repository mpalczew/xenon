//! Dev-only spike for `docs/designs/terminal-service.md`: one VTE with no PTY, fed
//! from the terminal service, input through `commit`. Never shipped.
//!
//! Start the service (`cargo run -p xenon_term --bin xenon-term`), then:
//!   cargo run -p xenon --example term-spike [-- [OPTIONS] [-- CMD ARGS...]]
//! It attaches to the first live terminal, or creates one (a shell, or CMD). Close the
//! window and run it again: the screen comes back from the service.
//!
//! Options: --log FILE (record every `commit`), --socket PATH, --new (always create), --send TEXT (type it after attaching;
//! understands \e \r \n \t \\ \xNN), --dump FILE (write the VTE text, cursor and title
//! after --after MS, default 1500, then quit), --size COLSxROWS (fix the window size).

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use vte::TerminalExt;
use xenon_term::{Client, Event, Size, Spawn, TermId};

#[derive(Default)]
struct Args {
    socket: Option<PathBuf>,
    fresh: bool,
    send: Option<String>,
    dump: Option<PathBuf>,
    log: Option<PathBuf>,
    after: u64,
    size: Option<(u16, u16)>,
    command: Vec<String>,
}

fn parse() -> Args {
    let mut args = Args {
        after: 1500,
        ..Args::default()
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--socket" => args.socket = it.next().map(PathBuf::from),
            "--new" => args.fresh = true,
            "--send" => args.send = it.next(),
            "--log" => args.log = it.next().map(PathBuf::from),
            "--dump" => args.dump = it.next().map(PathBuf::from),
            "--after" => args.after = it.next().and_then(|v| v.parse().ok()).unwrap_or(1500),
            "--size" => {
                args.size = it.next().and_then(|v| {
                    let (c, r) = v.split_once('x')?;
                    Some((c.parse().ok()?, r.parse().ok()?))
                });
            }
            "--" => {
                args.command = it.by_ref().collect();
            }
            other => eprintln!("term-spike: ignoring {other}"),
        }
    }
    args
}

fn unescape(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 >= bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        i += 2;
        match bytes[i - 1] {
            b'e' => out.push(0x1b),
            b'r' => out.push(b'\r'),
            b'n' => out.push(b'\n'),
            b't' => out.push(b'\t'),
            b'x' if i + 2 <= bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i..i + 2]).unwrap_or("00");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(0));
                i += 2;
            }
            other => out.push(other),
        }
    }
    out
}

fn vte_size(term: &vte::Terminal) -> Option<Size> {
    Size::new(
        u16::try_from(term.column_count()).ok()?,
        u16::try_from(term.row_count()).ok()?,
    )
}

/// Runs of visible cells that carry an OSC 8 link, as `row col..col uri`.
fn links(term: &vte::Terminal) -> String {
    let (cw, ch) = (term.char_width() as f64, term.char_height() as f64);
    let mut out = String::new();
    for row in 0..term.row_count() {
        let mut run: Option<(i64, String)> = None;
        for col in 0..=term.column_count() {
            let uri = (col < term.column_count())
                .then(|| term.check_hyperlink_at((col as f64 + 0.5) * cw, (row as f64 + 0.5) * ch))
                .flatten()
                .map(|u| u.to_string());
            if run.as_ref().map(|r| &r.1) != uri.as_ref() {
                if let Some((start, u)) = run.take() {
                    out.push_str(&format!("link {row} {start}..{} {u}\n", col - 1));
                }
                run = uri.map(|u| (col, u));
            }
        }
    }
    out
}

fn dump(term: &vte::Terminal, path: &std::path::Path) {
    let text = term.text_format(vte::Format::Text).unwrap_or_default();
    let html = term.text_format(vte::Format::Html).unwrap_or_default();
    let (col, row) = term.cursor_position();
    let title = term.termprop_string("xterm.title").0.unwrap_or_default();
    let body = format!(
        "size {}x{}\ncursor {col},{row}\ntitle {title}\n{}---\n{text}\n=== html\n{html}",
        term.column_count(),
        term.row_count(),
        links(term)
    );
    let _ = std::fs::write(path, body);
}

fn start(app: &gtk::Application, args: &Rc<Args>) {
    let socket = args
        .socket
        .clone()
        .unwrap_or_else(xenon_term::default_socket);
    let env: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    let svc = match Client::connect(&socket, env) {
        Ok(svc) => Rc::new(svc),
        Err(e) => {
            eprintln!("term-spike: {e}");
            std::process::exit(1);
        }
    };
    let term = vte::Terminal::new();
    term.set_scrollback_lines(10_000);
    term.set_allow_hyperlink(true);
    term.set_hexpand(true);
    term.set_vexpand(true);
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("term-spike")
        .default_width(900)
        .default_height(600)
        .child(&term)
        .build();
    // sync: the window title follows VTE's own title signal, no layout is running.
    term.connect_termprop_changed(Some("xterm.title"), {
        let window = window.clone();
        move |t, name| window.set_title(t.termprop_string(name).0.as_deref())
    });
    let id: Rc<Cell<Option<TermId>>> = Rc::default();
    // Everything VTE would write to a PTY comes out of `commit`: keys, paste, mouse and
    // focus reports, and its own replies to terminal queries.
    // sync: forwards bytes to the service on the same main-loop turn; nothing re-enters.
    term.connect_commit({
        let (svc, id, args) = (svc.clone(), id.clone(), args.clone());
        move |_, text, _| {
            if let Some(path) = &args.log {
                use std::io::Write as _;
                let line = format!("commit {:?}\n", text);
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .and_then(|mut f| f.write_all(line.as_bytes()));
            }
            if let Some(id) = id.get() {
                let _ = svc.input(id, text.as_bytes());
            }
        }
    });
    let events = svc.events();
    glib::spawn_future_local({
        let (term, id) = (term.clone(), id.clone());
        async move {
            while let Ok(ev) = events.recv().await {
                match ev {
                    Event::Output { id: from, bytes } if id.get() == Some(from) => {
                        term.feed(&bytes)
                    }
                    Event::Exited { exit, .. } => eprintln!("term-spike: child exited {exit:?}"),
                    Event::Lost => {
                        eprintln!("term-spike: service lost");
                        std::process::exit(1);
                    }
                    _ => {}
                }
            }
        }
    });
    window.present();
    attach_when_sized(&term, &svc, &id, args);
}

/// Wait for the first real size, attach with it, then follow size changes.
fn attach_when_sized(
    term: &vte::Terminal,
    svc: &Rc<Client>,
    id: &Rc<Cell<Option<TermId>>>,
    args: &Rc<Args>,
) {
    let last: RefCell<Option<Size>> = RefCell::default();
    let (svc, id, args) = (svc.clone(), id.clone(), args.clone());
    // sync: a per-frame size poll; it only reads VTE's size and talks to the socket.
    term.add_tick_callback(move |term, _| {
        if let Some((cols, rows)) = args.size
            && (term.column_count() != i64::from(cols) || term.row_count() != i64::from(rows))
        {
            term.set_size(i64::from(cols), i64::from(rows));
        }
        if term.width() <= 1 {
            return glib::ControlFlow::Continue;
        }
        let Some(size) = vte_size(term) else {
            return glib::ControlFlow::Continue;
        };
        if *last.borrow() == Some(size) {
            return glib::ControlFlow::Continue;
        }
        let first = last.replace(Some(size)).is_none();
        match id.get() {
            None if first => begin(term, &svc, &id, &args, size),
            Some(id) => {
                let _ = svc.resize(id, size);
            }
            None => {}
        }
        glib::ControlFlow::Continue
    });
}

fn begin(
    term: &vte::Terminal,
    svc: &Rc<Client>,
    id: &Rc<Cell<Option<TermId>>>,
    args: &Rc<Args>,
    size: Size,
) {
    let live = svc.list().unwrap_or_default();
    let found = live
        .iter()
        .find(|t| t.exit.is_none())
        .map(|t| t.id)
        .filter(|_| !args.fresh);
    let chosen = match found {
        Some(found) => found,
        None => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let spawn = Spawn {
                argv: args.command.clone(),
                ..Spawn::shell(&cwd)
            };
            match svc.create(spawn, size) {
                Ok(new) => new,
                Err(e) => {
                    eprintln!("term-spike: create failed: {e}");
                    std::process::exit(1);
                }
            }
        }
    };
    eprintln!(
        "term-spike: terminal {chosen} ({})",
        if found.is_some() { "reattached" } else { "new" }
    );
    id.set(Some(chosen));
    let _ = svc.attach(chosen, size);
    let (svc, args, term) = (svc.clone(), args.clone(), term.clone());
    glib::timeout_add_local_once(Duration::from_millis(600), move || {
        if let Some(text) = &args.send {
            let _ = svc.input(chosen, &unescape(text));
        }
        if let Some(path) = args.dump.clone() {
            glib::timeout_add_local_once(Duration::from_millis(args.after), move || {
                dump(&term, &path);
                std::process::exit(0);
            });
        }
    });
}

fn main() -> glib::ExitCode {
    let args = Rc::new(parse());
    let app = gtk::Application::builder()
        .application_id("dev.xenon.TermSpike")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    // sync: application activation builds the one window; no layout is running.
    app.connect_activate(move |app| start(app, &args));
    app.run_with_args::<&str>(&[])
}
