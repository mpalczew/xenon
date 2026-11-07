//! The cleaning rule behind Copy Clean: terminal text from an agent TUI with
//! control sequences, box and quote gutters, status suffixes and hard wraps
//! removed. No view owns it.

mod bar_gutter;

pub fn clean_agent_output(text: &str) -> String {
    let text = strip_terminal_control_sequences(text);
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    let content: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| !line.trim().is_empty())
        .collect();
    let quote_block = !content.is_empty()
        && content.iter().all(|line| {
            let line = line.trim_start();
            line == ">" || line.starts_with("> ")
        });
    if quote_block {
        for line in &mut lines {
            let trimmed = line.trim_start();
            *line = trimmed
                .strip_prefix("> ")
                .or_else(|| trimmed.strip_prefix('>'))
                .unwrap_or(trimmed)
                .to_string();
        }
    }
    let bar_block = bar_gutter::strip(&mut lines);
    let mut cleaned: Vec<(String, bool)> = lines
        .iter()
        .map(|line| {
            let (text, wrapped) = strip_box_gutters(line);
            (strip_status_suffix(&text), wrapped)
        })
        .collect();
    if bar_block {
        bar_gutter::mark_hard_wraps(&mut cleaned);
    }
    let mut lines = unwrap_box_lines(cleaned);
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Return fenced code blocks, or cleaned text when the selection has no fence.
pub fn extract_code(text: &str) -> String {
    let stripped = strip_terminal_control_sequences(text);
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut in_block = false;
    for line in stripped.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .chars()
            .next()
            .filter(|c| matches!(c, '│' | '┃' | '║' | '|'))
            .map(|border| trimmed[border.len_utf8()..].trim_start())
            .unwrap_or(trimmed);
        if marker.starts_with("```") || marker.starts_with("~~~") {
            if in_block {
                blocks.push(current.join("\n"));
            }
            current.clear();
            in_block = !in_block;
        } else if in_block {
            current.push(line);
        }
    }
    if in_block && !current.is_empty() {
        blocks.push(current.join("\n"));
    }
    if blocks.is_empty() {
        clean_agent_output(text)
    } else {
        blocks.join("\n\n")
    }
}

/// Strip one leading/trailing box-drawing gutter. `true` means the line filled
/// the TUI box (trailing border after padding) and the next line may wrap.
fn strip_box_gutters(line: &str) -> (String, bool) {
    let is_border = |ch: char| matches!(ch, '│' | '┃' | '║');
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return (String::new(), false);
    }
    if trimmed.chars().all(is_border) {
        return (String::new(), false);
    }

    let mut s = line;
    if let Some(ch) = s.trim_start().chars().next().filter(|c| is_border(*c)) {
        s = s.trim_start()[ch.len_utf8()..].trim_start();
    }
    let s = s.trim_end();
    let Some(ch) = s.chars().last().filter(|c| is_border(*c)) else {
        return (s.to_string(), false);
    };
    let without = &s[..s.len() - ch.len_utf8()];
    let wrapped = without.ends_with("  ");
    (without.trim().to_string(), wrapped)
}

fn unwrap_box_lines(lines: Vec<(String, bool)>) -> Vec<String> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for (text, wrapped) in lines {
        if text.is_empty() {
            if let Some(prev) = out.last_mut() {
                prev.1 = false;
            }
            out.push((text, false));
            continue;
        }
        if let Some(prev) = out.last_mut()
            && prev.1
            && !prev.0.is_empty()
        {
            prev.0.push(' ');
            prev.0.push_str(&text);
            prev.1 = wrapped;
            continue;
        }
        out.push((text, wrapped));
    }
    out.into_iter().map(|(text, _)| text).collect()
}

fn strip_terminal_control_sequences(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&']') {
                chars.next();
                while let Some(ch) = chars.next() {
                    if ch == '\u{7}' {
                        break;
                    }
                    if ch == '\u{1b}' && chars.next_if_eq(&'\\').is_some() {
                        break;
                    }
                }
            } else {
                let _ = chars.next_if_eq(&'[');
                for ch in chars.by_ref() {
                    if ('@'..='~').contains(&ch) {
                        break;
                    }
                }
            }
        } else if !ch.is_control() || matches!(ch, '\n' | '\r' | '\t') {
            result.push(ch);
        }
    }
    result.replace('\r', "")
}

fn strip_status_suffix(line: &str) -> String {
    let without_cursor = line
        .trim_end()
        .trim_end_matches(['█', '▌', '▋', '▊', '▉'])
        .trim_end();
    let Some((prefix, suffix)) = without_cursor.rsplit_once("  ") else {
        return without_cursor.to_string();
    };
    let time = suffix
        .strip_suffix(" AM")
        .or_else(|| suffix.strip_suffix(" PM"))
        .unwrap_or(suffix);
    let looks_like_time = time.split_once(':').is_some_and(|(hour, minute)| {
        !hour.is_empty()
            && !minute.is_empty()
            && hour.chars().all(|c| c.is_ascii_digit())
            && minute.chars().take(2).all(|c| c.is_ascii_digit())
    });
    if looks_like_time {
        prefix.trim_end().to_string()
    } else {
        without_cursor.to_string()
    }
}
