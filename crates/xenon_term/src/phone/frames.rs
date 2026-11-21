//! A phone's frame thread: at most one frame per gap, built from the emulator under the
//! terminal's lock and encoded outside it.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};

use xenon_remote::{FrameEncoder, Outbox, ServerMsg};

use super::colors::Look;
use super::session::View;
use super::{Phone, Watch};
use crate::emu_view;
use crate::term::Term;

const GAP: Duration = Duration::from_millis(33);
const LOW_GAP: Duration = Duration::from_millis(100);
const RETRY_AFTER: Duration = Duration::from_millis(250);

pub(super) fn start(
    phone: &Phone,
    term: &Arc<Term>,
    watch: &Arc<Watch>,
    view: &Arc<View>,
    out: Outbox,
) {
    let (term, watch, view) = (Arc::clone(term), Arc::clone(watch), Arc::clone(view));
    let phone = phone.self_ref();
    let spawned = thread::Builder::new()
        .name("xenon-phone-frames".into())
        .spawn(move || run(&phone, &term, &watch, &view, &out));
    if let Err(error) = spawned {
        log::error!("phone frame thread: {error}");
    }
}

fn run(phone: &Weak<Phone>, term: &Term, watch: &Watch, view: &View, out: &Outbox) {
    let mut encoder = FrameEncoder::default();
    let mut last = Instant::now();
    let mut shown_dark = None;
    loop {
        let gap = if view.low_bandwidth.load(Ordering::Relaxed) {
            LOW_GAP
        } else {
            GAP
        };
        if !watch.wait(last, gap) {
            return;
        }
        last = Instant::now();
        let Some(phone) = phone.upgrade() else { return };
        let dark = phone.dark(view.look.load(Ordering::Relaxed));
        if shown_dark.replace(dark).is_some_and(|d| d != dark) {
            let theme = xenon_remote::theme_wire(Look::of(dark).theme());
            out.try_send(ServerMsg::Theme { theme });
            view.full.store(true, Ordering::Relaxed);
        }
        if view.full.swap(false, Ordering::Relaxed) {
            encoder.force_full();
        }
        let offset = view.offset.load(Ordering::Relaxed);
        let screen = emu_view::screen(&term.lock().emu, offset, &Look::of(dark));
        let delivered = encoder.encode(&screen).into_iter().all(|m| out.try_send(m));
        if !delivered {
            encoder.force_full();
            thread::sleep(RETRY_AFTER);
            watch.poke();
        }
    }
}
