//! Terminal tab titles: what to show for the title a program set.

/// Title text the terminal reports before any program sets one.
const PLACEHOLDER: &str = "terminal";

/// Clean a raw terminal title. The default title is `{cwd} — {process}`; drop
/// the cwd prefix only while it still equals `spawn_dir` (the name of the
/// directory the terminal started in, which the sidebar already shows). A
/// cd'd-into subdir is kept, as is a program-set title. Empty becomes "terminal".
pub fn clean_terminal_title(raw: &str, spawn_dir: &str) -> String {
    let title = match raw.split_once(" — ") {
        Some((dir, rest)) if dir == spawn_dir => rest,
        _ => raw,
    };
    if title.is_empty() {
        PLACEHOLDER.into()
    } else {
        title.to_string()
    }
}

/// Label for a terminal tab chip. Plain shell names repeat, so they get the
/// tab's 1-based position; any other title is shown as is.
pub fn terminal_tab_label(title: &str, index: usize) -> String {
    if matches!(title, "bash" | "zsh" | "fish") {
        format!("{title} · {}", index + 1)
    } else {
        title.to_string()
    }
}
