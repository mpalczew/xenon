use std::time::Duration;

pub const IDLE_AFTER: Duration = Duration::from_millis(2500);
pub(crate) const BUSY_WAKEUPS: u32 = 15;
pub(crate) const ANONYMOUS_BUSY_WAKEUPS: u32 = 40;

pub(crate) fn agent_busy_signal(busy: u32, title: &str) -> bool {
    if busy == 0 {
        return false;
    }
    if agent_title(title) {
        return busy >= BUSY_WAKEUPS;
    }
    busy >= ANONYMOUS_BUSY_WAKEUPS
}

pub(crate) fn agent_title(title: &str) -> bool {
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
