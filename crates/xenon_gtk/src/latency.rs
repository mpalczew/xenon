//! Optional latency log, on when `XENON_GTK_LATENCY` is set. It stamps the
//! moment input reaches the terminal, then logs how long VTE took to report
//! new contents, how long our handler ran, and when the next frame was
//! painted. Off, it costs one cached bool check per call.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::Instant;

use gtk::prelude::*;

fn stamp() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3
}

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("XENON_GTK_LATENCY").is_some())
}

thread_local! {
    static INPUT: Cell<Option<Instant>> = const { Cell::new(None) };
    /// (input time, contents-changed time) waiting for the frame that shows it.
    static PENDING: Cell<Option<(Instant, Instant)>> = const { Cell::new(None) };
    static LAST_PAINT: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Input just went to the terminal (key press or injected text).
pub(crate) fn input() {
    if enabled() {
        INPUT.with(|i| i.set(Some(Instant::now())));
    }
}

/// Run `handler` (our `contents-changed` work) and log its duration.
pub(crate) fn contents_changed(handler: impl FnOnce()) {
    if !enabled() {
        return handler();
    }
    let start = Instant::now();
    if std::env::var("XENON_GTK_LATENCY").is_ok_and(|v| v == "trace") {
        eprintln!("[tr] {:8.2} contents-changed", stamp());
    }
    handler();
    let spent = start.elapsed();
    let since_input = INPUT.with(|i| i.get()).map(|t| start.duration_since(t));
    eprintln!("[lat] contents-changed since_input={since_input:?} handler={spent:?}");
    if let Some(input) = INPUT.with(|i| i.take()) {
        PENDING.with(|p| p.set(Some((input, start))));
    }
}

/// Log each painted frame that follows input.
pub(crate) fn watch_frames(widget: &impl IsA<gtk::Widget>) {
    if !enabled() {
        return;
    }
    widget.connect_realize(|w| {
        let Some(clock) = w
            .native()
            .and_then(|n| n.surface())
            .map(|s| s.frame_clock())
        else {
            return;
        };
        clock.connect_before_paint(|_| LAST_PAINT.with(|p| p.set(Some(Instant::now()))));
        if std::env::var("XENON_GTK_LATENCY").is_ok_and(|v| v == "trace") {
            for (name, hook) in [
                ("flush", 0),
                ("before", 1),
                ("update", 2),
                ("layout", 3),
                ("paint", 4),
                ("after", 5),
            ] {
                let log = move || eprintln!("[tr] {:8.2} {name}", stamp());
                match hook {
                    0 => clock.connect_flush_events(move |_| log()),
                    1 => clock.connect_before_paint(move |_| log()),
                    2 => clock.connect_update(move |_| log()),
                    3 => clock.connect_layout(move |_| log()),
                    4 => clock.connect_paint(move |_| log()),
                    _ => clock.connect_after_paint(move |_| log()),
                };
            }
        }
        let widget = w.clone();
        let announced = Cell::new(false);
        clock.connect_after_paint(move |_| {
            if !announced.replace(true) {
                eprintln!(
                    "[lat] window {}x{} scale {}",
                    widget.width(),
                    widget.height(),
                    widget.scale_factor()
                );
            }
            let now = Instant::now();
            let began = LAST_PAINT.with(|p| p.get());
            let shown = PENDING
                .with(|p| p.get())
                .filter(|(_, cc)| began.is_some_and(|b| b >= *cc));
            if let Some((input, _)) = shown {
                PENDING.with(|p| p.set(None));
                eprintln!(
                    "[lat] frame since_input={:?} paint={:?}",
                    now.duration_since(input),
                    began.map(|b| now.duration_since(b))
                );
            }
        });
    });
}
