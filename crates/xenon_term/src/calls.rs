//! Fire-and-forget calls: no answer comes back.

use xenon_core::TermId;

use crate::wire::Request;
use crate::{Client, Size};

impl Client {
    /// Become this terminal's window at `size`. A replay arrives first as
    /// `Event::Output`, then live output. Attaching again replays again.
    pub fn attach(&self, id: TermId, size: Size) {
        self.send(0, &Request::Attach { id, size });
    }

    /// Bytes for the child: keys, pastes, mouse reports and the view's replies to queries.
    pub fn input(&self, id: TermId, bytes: &[u8]) {
        self.send(
            0,
            &Request::Input {
                id,
                bytes: bytes.to_vec(),
            },
        );
    }

    /// The view's own size changed.
    pub fn claim(&self, id: TermId, size: Size) {
        self.send(0, &Request::Claim { id, size });
    }

    /// The tab is on screen: clears its attention.
    pub fn seen(&self, id: TermId) {
        self.send(0, &Request::Seen { id });
    }

    /// Hang up the child. Idempotent.
    pub fn close(&self, id: TermId) {
        self.send(0, &Request::Close { id });
    }

    /// Whether the window's theme is dark; the phone follows it unless set otherwise.
    pub fn set_dark(&self, dark: bool) {
        self.send(0, &Request::Dark { dark });
    }
}
