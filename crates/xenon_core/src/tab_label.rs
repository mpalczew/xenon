const PLACEHOLDER: &str = "terminal";

/// Clean a raw terminal title. The default title is `{cwd} — {process}`; drop
/// the cwd prefix only while it still equals `spawn_dir` (the name of the
/// directory the terminal started in, which the sidebar already shows). A
/// cd'd-into subdir is kept, as is a program-set title. Empty becomes "terminal".
pub fn clean_terminal_title(raw: &str, spawn_dir: &str) -> String {
    if let Some(path) = prompt_path(raw) {
        return match short_dir(path) {
            name if name == spawn_dir || name.is_empty() => PLACEHOLDER.into(),
            name => name,
        };
    }
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

fn prompt_path(raw: &str) -> Option<&str> {
    let (who, path) = raw.split_once(':')?;
    let (user, host) = who.split_once('@')?;
    let word = |s: &str| !s.is_empty() && !s.contains(char::is_whitespace);
    (word(user) && word(host)).then(|| path.trim())
}

fn short_dir(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() {
            String::new()
        } else {
            "/".into()
        };
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed).to_string()
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
