use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};

use crate::preload::{self, Depth};
use crate::remote::{Outbox, ServerMsg, WindowEncoder};
use xenon_term::Mirror;

use crate::look::Look;

use super::Phone;
use super::session::View;

const GAP: Duration = Duration::from_millis(33);
const LOW_GAP: Duration = Duration::from_millis(100);
const RETRY_AFTER: Duration = Duration::from_millis(250);

pub(super) fn start(phone: &Phone, mirror: &Arc<Mirror>, view: &Arc<View>, out: Outbox) {
    let (mirror, view) = (Arc::clone(mirror), Arc::clone(view));
    let phone = phone.self_ref();
    let spawned = thread::Builder::new()
        .name("xenon-phone-frames".into())
        .spawn(move || run(&phone, &mirror, &view, &out));
    if let Err(error) = spawned {
        log::error!("phone frame thread: {error}");
    }
}

fn run(phone: &Weak<Phone>, mirror: &Mirror, view: &View, out: &Outbox) {
    let mut encoder = WindowEncoder::default();
    let mut last = Instant::now();
    let mut shown_dark = None;
    loop {
        let low = view.low_bandwidth.load(Ordering::Relaxed);
        let gap = if low { LOW_GAP } else { GAP };
        if !mirror.wait_dirty(last, gap) {
            if mirror.is_gone() {
                eprintln!("xenon-phone frames stopped: terminal connection gone");
            }
            return;
        }
        last = Instant::now();
        let Some(phone) = phone.upgrade() else { return };
        let dark = phone.dark(view.look.load(Ordering::Relaxed));
        let look = Look::of(dark);
        if shown_dark.replace(dark).is_some_and(|d| d != dark) {
            out.try_send(ServerMsg::Theme {
                theme: look.theme(),
            });
            view.full.store(true, Ordering::Relaxed);
        }
        if view.full.swap(false, Ordering::Relaxed) {
            encoder.force_full();
        }
        let report = view.report();
        let rendered = mirror.read(|term, at| {
            let want = preload::plan(at, report, encoder.held(), Depth::of(low));
            let holds = |n| encoder.holds_history(n, at);
            crate::screen::render(term, at, &want, holds, &look)
        });
        let delivered = encoder
            .encode(&rendered)
            .into_iter()
            .all(|m| out.try_send(m));
        if !delivered {
            encoder.force_full();
            thread::sleep(RETRY_AFTER);
            mirror.wake();
        }
    }
}
