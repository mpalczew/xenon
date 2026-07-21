//! Vim-style `:s/pattern/replacement/flags` parse, expand, and apply.
//!
//! String specials only (`:h sub-replace-special`): `&` `\0`…`\9`, case escapes,
//! `\r`/`\t`/`\n`, `~`. No `\=` expression register.

use regex::{Captures, RegexBuilder};

use crate::buffer::Buffer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubstituteCmd {
    /// `None` = reuse last search pattern.
    pub pattern: Option<String>,
    /// Raw vim replacement (expanded per match).
    pub replacement: String,
    pub flags: SubstFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubstFlags {
    pub all: bool,
    pub ignore_case: bool,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubstituteOutcome {
    pub replacements: usize,
    pub lines_changed: usize,
}

/// Parse the body after `s` / `substitute` (must start with `/`).
pub fn parse_substitute(s: &str) -> Result<SubstituteCmd, String> {
    let rest = s
        .strip_prefix('/')
        .ok_or_else(|| "substitute: expected /pattern/replacement/".to_string())?;
    let parts = split_on_slash(rest);
    if parts.len() < 2 {
        return Err("substitute needs /pattern/replacement/".into());
    }
    let pattern = if parts[0].is_empty() {
        None
    } else {
        Some(parts[0].clone())
    };
    let replacement = parts[1].clone();
    let mut flags = SubstFlags::default();
    for ch in parts.get(2).map(String::as_str).unwrap_or("").chars() {
        match ch {
            'g' => flags.all = true,
            'i' => flags.ignore_case = true,
            'I' => flags.case_sensitive = true,
            ' ' | '\t' => {}
            other => return Err(format!("unknown substitute flag '{other}'")),
        }
    }
    Ok(SubstituteCmd {
        pattern,
        replacement,
        flags,
    })
}

/// Apply substitute on inclusive 0-based line range.
pub fn apply_substitute(
    buffer: &mut Buffer,
    cmd: &SubstituteCmd,
    lines: (usize, usize),
    last_search: Option<&str>,
    last_replacement: &mut String,
) -> Result<SubstituteOutcome, String> {
    let (first_line, last_line) = lines;
    let pattern = match &cmd.pattern {
        Some(p) => p.as_str(),
        None => last_search.ok_or_else(|| "no previous regular expression".to_string())?,
    };
    let ignore_case = if cmd.flags.case_sensitive {
        false
    } else {
        cmd.flags.ignore_case
    };
    let re = RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| e.to_string())?;

    let n_lines = buffer.rope().len_lines().max(1);
    let first = first_line.min(n_lines.saturating_sub(1));
    let last = last_line.min(n_lines.saturating_sub(1)).max(first);

    // Collect (char_start, char_end, replacement) in reverse so edits stay valid.
    let mut ops: Vec<(usize, usize, String)> = Vec::new();
    let mut replacements = 0usize;
    let mut lines_changed = 0usize;
    let prev = last_replacement.clone();

    for row in first..=last {
        let line_start = buffer.rope().line_to_char(row);
        let line_end = if row + 1 < n_lines {
            buffer.rope().line_to_char(row + 1)
        } else {
            buffer.rope().len_chars()
        };
        // Work on line content without trailing newline for regex, but map offsets.
        let full = buffer.rope().slice(line_start..line_end).to_string();
        let (content, nl) = if full.ends_with('\n') {
            (&full[..full.len() - 1], true)
        } else {
            (full.as_str(), false)
        };
        if content.is_empty() && !re.is_match("") {
            continue;
        }
        let mut line_ops = Vec::new();
        let mut changed = false;
        if cmd.flags.all {
            for caps in re.captures_iter(content) {
                let m = caps.get(0).unwrap();
                let rep = expand_replacement(&cmd.replacement, &caps, &prev);
                let start = line_start + content[..m.start()].chars().count();
                let end = start + content[m.start()..m.end()].chars().count();
                line_ops.push((start, end, rep));
            }
        } else if let Some(caps) = re.captures(content) {
            let m = caps.get(0).unwrap();
            let rep = expand_replacement(&cmd.replacement, &caps, &prev);
            let start = line_start + content[..m.start()].chars().count();
            let end = start + content[m.start()..m.end()].chars().count();
            line_ops.push((start, end, rep));
        }
        if !line_ops.is_empty() {
            changed = true;
            replacements += line_ops.len();
            ops.extend(line_ops);
        }
        let _ = (nl, changed);
        if changed {
            lines_changed += 1;
        }
    }

    if ops.is_empty() {
        return Ok(SubstituteOutcome::default());
    }

    // Apply right-to-left; one undo group for the whole :s.
    buffer.set_undo_group(true);
    ops.sort_by_key(|b| std::cmp::Reverse(b.0));
    for (start, end, rep) in ops {
        buffer.replace_range(start..end, &rep);
    }
    buffer.set_undo_group(false);

    *last_replacement = cmd.replacement.clone();
    Ok(SubstituteOutcome {
        replacements,
        lines_changed,
    })
}

fn split_on_slash(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('/') => cur.push('/'),
                Some(other) => {
                    cur.push('\\');
                    cur.push(other);
                }
                None => cur.push('\\'),
            }
        } else if ch == '/' {
            parts.push(std::mem::take(&mut cur));
            if parts.len() == 2 {
                // Rest is flags (may contain no more structural slashes needed).
                cur.extend(chars);
                parts.push(cur);
                return parts;
            }
        } else {
            cur.push(ch);
        }
    }
    parts.push(cur);
    parts
}

/// Expand vim replacement specials against one match.
pub fn expand_replacement(raw: &str, caps: &Captures, prev: &str) -> String {
    let mut out = String::new();
    expand_into(&mut out, raw, caps, prev, true);
    out
}

#[derive(Clone, Copy)]
enum CaseState {
    None,
    UpperOne,
    LowerOne,
    UpperRun,
    LowerRun,
}

fn push_cased(out: &mut String, case: &mut CaseState, ch: char) {
    match *case {
        CaseState::None => out.push(ch),
        CaseState::UpperOne => {
            out.extend(ch.to_uppercase());
            *case = CaseState::None;
        }
        CaseState::LowerOne => {
            out.extend(ch.to_lowercase());
            *case = CaseState::None;
        }
        CaseState::UpperRun => out.extend(ch.to_uppercase()),
        CaseState::LowerRun => out.extend(ch.to_lowercase()),
    }
}

fn expand_into(out: &mut String, raw: &str, caps: &Captures, prev: &str, allow_tilde: bool) {
    let mut case = CaseState::None;
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some('\\') => push_cased(out, &mut case, '\\'),
                Some('&') => push_cased(out, &mut case, '&'),
                Some('~') => push_cased(out, &mut case, '~'),
                Some('r') => push_cased(out, &mut case, '\r'),
                Some('t') => push_cased(out, &mut case, '\t'),
                // Vim: \n in replacement is NUL.
                Some('n') => push_cased(out, &mut case, '\0'),
                Some('0') => push_group(out, &mut case, caps, 0),
                Some(d) if d.is_ascii_digit() => {
                    let n = (d as u8 - b'0') as usize;
                    push_group(out, &mut case, caps, n);
                }
                Some('u') => case = CaseState::UpperOne,
                Some('l') => case = CaseState::LowerOne,
                Some('U') => case = CaseState::UpperRun,
                Some('L') => case = CaseState::LowerRun,
                Some('E' | 'e') => case = CaseState::None,
                Some(other) => push_cased(out, &mut case, other),
                None => push_cased(out, &mut case, '\\'),
            },
            '&' => push_group(out, &mut case, caps, 0),
            '~' if allow_tilde => {
                // Re-expand previous replacement against this match (no nested ~).
                expand_into(out, prev, caps, "", false);
            }
            other => push_cased(out, &mut case, other),
        }
    }
}

fn push_group(out: &mut String, case: &mut CaseState, caps: &Captures, n: usize) {
    let text = caps.get(n).map(|m| m.as_str()).unwrap_or("");
    for ch in text.chars() {
        push_cased(out, case, ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use regex::Regex;
    use tempfile::NamedTempFile;

    fn buffer_with(text: &str) -> Buffer {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        Buffer::open(file.path()).unwrap()
    }

    #[test]
    fn parse_basic() {
        let cmd = parse_substitute("/foo/bar/gi").unwrap();
        assert_eq!(cmd.pattern.as_deref(), Some("foo"));
        assert_eq!(cmd.replacement, "bar");
        assert!(cmd.flags.all && cmd.flags.ignore_case);
    }

    #[test]
    fn expand_groups_and_case() {
        let re = Regex::new(r"(\w+)").unwrap();
        let caps = re.captures("hello").unwrap();
        assert_eq!(expand_replacement(r"<<\1>>", &caps, ""), "<<hello>>");
        assert_eq!(expand_replacement(r"\U&\E", &caps, ""), "HELLO");
        assert_eq!(expand_replacement(r"\u&", &caps, ""), "Hello");
    }

    #[test]
    fn apply_global_on_percent() {
        let mut buf = buffer_with("foo foo\nbar foo\n");
        let cmd = parse_substitute("/foo/X/g").unwrap();
        let mut prev = String::new();
        let out = apply_substitute(&mut buf, &cmd, (0, 1), None, &mut prev).unwrap();
        assert_eq!(out.replacements, 3);
        assert_eq!(buf.text(), "X X\nbar X\n");
    }

    #[test]
    fn apply_capture_replace() {
        let mut buf = buffer_with("ab cd\n");
        let cmd = parse_substitute(r"/(\w+)/[\1]/g").unwrap();
        let mut prev = String::new();
        apply_substitute(&mut buf, &cmd, (0, 0), None, &mut prev).unwrap();
        assert_eq!(buf.text(), "[ab] [cd]\n");
    }
}
