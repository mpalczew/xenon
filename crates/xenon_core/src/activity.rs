use crate::agent::agent_busy_signal;

#[derive(Clone, Debug, Default)]
pub struct TerminalActivity {
    wakeups: u32,
    working: bool,
}

impl TerminalActivity {
    pub fn is_working(&self) -> bool {
        self.working
    }

    /// One batch of output. True when this batch made the terminal "working".
    pub fn on_output(&mut self, title: &str) -> bool {
        self.wakeups = self.wakeups.saturating_add(1);
        if !self.working && agent_busy_signal(self.wakeups, title) {
            self.working = true;
            return true;
        }
        false
    }

    /// Output has been quiet for `IDLE_AFTER`. True when the burst was
    /// agent-sized (the terminal was working, or the burst qualifies now).
    pub fn on_quiet(&mut self, title: &str) -> bool {
        let busy = std::mem::take(&mut self.wakeups);
        let was_working = std::mem::take(&mut self.working);
        was_working || agent_busy_signal(busy, title)
    }

    /// The user typed here: their echo must not count as agent output.
    /// True when this ended a working state.
    pub fn on_interaction(&mut self) -> bool {
        self.wakeups = 0;
        std::mem::take(&mut self.working)
    }

    pub fn on_exit(&mut self) {
        self.wakeups = 0;
        self.working = false;
    }
}
