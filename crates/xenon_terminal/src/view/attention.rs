//! Detect agent-like work from PTY wakeup cadence.
//!
//! Named harness titles need fewer wakeups; anonymous shells need a heavier burst
//! so `npm install` noise does not light the workspace working / done dots.

use std::time::Duration;

/// Output must be quiet this long before a terminal counts as settled.
pub(super) const IDLE_AFTER: Duration = Duration::from_millis(2500);
/// Named-harness title: settled burst this large counts as agent work.
pub(super) const BUSY_WAKEUPS: u32 = 15;
/// No harness name in title: require a heavier burst.
pub(super) const ANONYMOUS_BUSY_WAKEUPS: u32 = 40;

/// Burst this large counts as agent work: pulse while output continues, then
/// the settle timer marks the workspace done.
pub(super) fn agent_busy_signal(busy: u32, title: &str) -> bool {
    if busy == 0 {
        return false;
    }
    if agent_title(title) {
        return busy >= BUSY_WAKEUPS;
    }
    busy >= ANONYMOUS_BUSY_WAKEUPS
}

/// Title (or process name fragment) suggests a coding agent, not a plain shell.
pub(super) fn agent_title(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    const NAMES: &[&str] = &[
        "claude", "grok", "kimi", "codex", "aider", "gemini", "cursor", "opencode", "windsurf",
        "goose", "crush", "amp ", " amp", "devin", "copilot",
    ];
    if NAMES.iter().any(|n| t.contains(n)) {
        return true;
    }
    t.contains("agent") || t.contains("llm")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_busy_detects_named_and_anonymous_bursts() {
        assert!(!agent_busy_signal(5, "claude"));
        assert!(agent_busy_signal(15, "claude code"));
        assert!(agent_busy_signal(20, "Grok session"));
        assert!(!agent_busy_signal(20, "bash"));
        assert!(agent_busy_signal(40, "bash"));
        assert!(agent_title("kimi-cli"));
        assert!(!agent_title("zsh"));
    }
}
