//! Fire-and-forget calls from the window: no answer comes back.

use xenon_core::TermId;

use crate::proto::v1::ToHolder;
use crate::{Size, Terminals, builds};

impl Terminals {
    /// Become this terminal's window at `size`. A replay arrives first as
    /// `Event::Output`, then live output. Attaching again replays again.
    pub fn attach(&self, id: TermId, size: Size) {
        self.shared.to_link_or_early(
            id,
            |link| {
                link.tell(&ToHolder::Claim { size, label: None });
                link.tell(&ToHolder::View { answers: true });
            },
            |early| {
                early.size = Some(size);
                early.attach = true;
            },
        );
    }

    /// Bytes for the child: keys, pastes, mouse reports and the view's replies to queries.
    pub fn input(&self, id: TermId, bytes: &[u8]) {
        self.shared.to_link_or_early(
            id,
            |link| {
                if link.speaks {
                    link.send_data(bytes);
                }
            },
            |early| early.input.extend_from_slice(bytes),
        );
    }

    /// The view's own size changed.
    pub fn claim(&self, id: TermId, size: Size) {
        self.shared.to_link_or_early(
            id,
            |link| link.tell(&ToHolder::Claim { size, label: None }),
            |early| early.size = Some(size),
        );
    }

    /// The tab is on screen: clears its attention.
    pub fn seen(&self, id: TermId) {
        if let Some(link) = self.shared.link(id) {
            link.tell(&ToHolder::Seen);
        }
    }

    /// The label of the build new terminals start on.
    pub fn installed_build(&self) -> String {
        builds::installed_label()
    }
}
