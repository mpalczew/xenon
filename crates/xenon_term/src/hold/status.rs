use std::time::{Duration, Instant};

use xenon_core::{AttentionReason, IDLE_AFTER, TerminalActivity, clean_terminal_title};

use crate::{Exit, TermStatus};

const WAKEUP_GAP: Duration = Duration::from_millis(16);

pub(crate) struct Status {
    activity: TerminalActivity,
    attention: Option<AttentionReason>,
    title: String,
    spawn_dir: String,
    exit: Option<Exit>,
    phone: Option<String>,
    last_wakeup: Option<Instant>,
    last_output: Option<Instant>,
    quiet_armed: bool,
}

impl Status {
    pub(crate) fn new(spawn_dir: String) -> Self {
        Self {
            activity: TerminalActivity::default(),
            attention: None,
            title: "terminal".into(),
            spawn_dir,
            exit: None,
            phone: None,
            last_wakeup: None,
            last_output: None,
            quiet_armed: false,
        }
    }

    pub(crate) fn snapshot(&self) -> TermStatus {
        TermStatus {
            working: self.activity.is_working(),
            attention: self.attention,
            title: self.title.clone(),
            exit: self.exit,
            phone: self.phone.clone(),
        }
    }

    pub(crate) fn on_output(&mut self, now: Instant) -> bool {
        self.last_output = Some(now);
        self.quiet_armed = true;
        if self
            .last_wakeup
            .is_some_and(|at| now.duration_since(at) < WAKEUP_GAP)
        {
            return false;
        }
        self.last_wakeup = Some(now);
        self.activity.on_output(&self.title)
    }

    pub(crate) fn quiet_deadline(&self) -> Option<Instant> {
        self.last_output
            .filter(|_| self.quiet_armed)
            .map(|at| at + IDLE_AFTER)
    }

    pub(crate) fn on_quiet(&mut self, now: Instant) -> bool {
        if self.quiet_deadline().is_none_or(|at| now < at) {
            return false;
        }
        self.quiet_armed = false;
        let was_working = self.activity.is_working();
        let finished = self.activity.on_quiet(&self.title);
        let flagged = finished && self.set_attention(Some(AttentionReason::IdleSettled));
        was_working || flagged
    }

    pub(crate) fn on_bell(&mut self) -> bool {
        self.set_attention(Some(AttentionReason::Bell))
    }

    pub(crate) fn on_input(&mut self) -> bool {
        let was_working = self.activity.on_interaction();
        self.set_attention(None) || was_working
    }

    pub(crate) fn seen(&mut self) -> bool {
        self.set_attention(None)
    }

    pub(crate) fn on_exit(&mut self, exit: Exit) {
        self.activity.on_exit();
        self.exit = Some(exit);
        self.quiet_armed = false;
    }

    pub(crate) fn set_phone(&mut self, label: Option<String>) -> bool {
        if self.phone == label {
            return false;
        }
        self.phone = label;
        true
    }

    pub(crate) fn set_title(&mut self, raw: Option<&str>) -> bool {
        let raw = raw.unwrap_or("");
        let mut clean = clean_terminal_title(raw, &self.spawn_dir);
        if clean == "terminal" && !raw.is_empty() {
            clean = shell_name();
        }
        if clean == self.title {
            return false;
        }
        self.title = clean;
        true
    }

    fn set_attention(&mut self, reason: Option<AttentionReason>) -> bool {
        if self.attention == reason {
            return false;
        }
        self.attention = reason;
        true
    }
}

fn shell_name() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "shell".into())
}
