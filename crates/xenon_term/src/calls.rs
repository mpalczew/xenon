use xenon_core::TermId;

use crate::links::Call;
use crate::{Size, Terminals};

impl Terminals {
    /// Become this terminal's window at `size`. A replay arrives first as
    /// `Event::Output`, then live output. Attaching again replays again.
    pub fn attach(&self, id: TermId, size: Size) {
        self.shared.links.send(id, Call::Attach(size));
    }

    /// Bytes for the child: keys, pastes, mouse reports and the view's replies to queries.
    pub fn input(&self, id: TermId, bytes: &[u8]) {
        self.shared.links.send(id, Call::Input(bytes.to_vec()));
    }

    pub fn claim(&self, id: TermId, size: Size) {
        self.shared.links.send(id, Call::Claim(size));
    }

    pub fn seen(&self, id: TermId) {
        self.shared.links.send(id, Call::Seen);
    }
}
