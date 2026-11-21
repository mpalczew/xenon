//! Fire-and-forget calls from the window: no answer comes back.

use xenon_core::TermId;

use crate::proto::v1::ToHolder;
use crate::terminals::Early;
use crate::{Size, Terminals, builds};

impl Terminals {
    /// Become this terminal's window at `size`. A replay arrives first as
    /// `Event::Output`, then live output. Attaching again replays again.
    pub fn attach(&self, id: TermId, size: Size) {
        if let Some(link) = self.shared.link(id) {
            link.tell(&ToHolder::Claim { size, label: None });
            link.tell(&ToHolder::View { answers: true });
        } else {
            self.early(id, |e| {
                e.size = Some(size);
                e.attach = true;
            });
        }
    }

    /// Bytes for the child: keys, pastes, mouse reports and the view's replies to queries.
    pub fn input(&self, id: TermId, bytes: &[u8]) {
        match self.shared.link(id) {
            Some(link) if link.speaks => link.send_data(bytes),
            Some(_) => {}
            None => self.early(id, |e| e.input.extend_from_slice(bytes)),
        }
    }

    /// The view's own size changed.
    pub fn claim(&self, id: TermId, size: Size) {
        if let Some(link) = self.shared.link(id) {
            link.tell(&ToHolder::Claim { size, label: None });
        } else {
            self.early(id, |e| e.size = Some(size));
        }
    }

    /// The tab is on screen: clears its attention.
    pub fn seen(&self, id: TermId) {
        if let Some(link) = self.shared.link(id) {
            link.tell(&ToHolder::Seen);
        }
    }

    /// Remember what happens to a terminal that is still starting.
    fn early(&self, id: TermId, change: impl FnOnce(&mut Early)) {
        let mut early = self.shared.early.lock().unwrap_or_else(|e| e.into_inner());
        change(early.entry(id).or_default());
    }

    /// The label of the build new terminals start on.
    pub fn installed_build(&self) -> String {
        builds::installed_label()
    }
}
