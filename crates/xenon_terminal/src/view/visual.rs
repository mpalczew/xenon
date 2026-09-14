//! Visual-test hooks for seeding deterministic terminal chrome.

use gpui::{Context, Pixels, Point};

use super::{State, TerminalView};

impl TerminalView {
    pub fn visual_set_working(&mut self, working: bool, cx: &mut Context<Self>) {
        self.working = working;
        cx.notify();
    }

    pub fn visual_open_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.context_menu = Some(position);
        cx.notify();
    }

    pub fn visual_is_ready(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }
}
