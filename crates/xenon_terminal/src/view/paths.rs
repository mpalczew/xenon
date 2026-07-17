//! Resolve cmd-clicked / right-clicked path-like tokens in the terminal grid.

use std::path::PathBuf;

/// Resolve a cmd-clicked path-like target to an existing file: strip any trailing
/// `:line[:col]` and resolve relative paths against the terminal's directory.
pub(super) fn resolve_clicked_path(target: &terminal::PathLikeTarget) -> Option<PathBuf> {
    for candidate in [
        target.maybe_path.as_str(),
        strip_line_suffix(&target.maybe_path),
    ] {
        let mut path = PathBuf::from(candidate);
        if path.is_relative() {
            match &target.terminal_dir {
                Some(dir) => path = dir.join(path),
                None => continue,
            }
        }
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// The whitespace-delimited token at grid cell `(line, col)`, trimmed of wrapping
/// brackets/quotes and trailing sentence punctuation.
pub(super) fn word_at(content: &terminal::Content, line: i32, col: usize) -> Option<String> {
    let num_cols = content.terminal_bounds.num_columns();
    if col >= num_cols {
        return None;
    }
    let mut row = vec![' '; num_cols];
    for indexed in &content.cells {
        if indexed.point.line == line && indexed.point.column < num_cols {
            row[indexed.point.column] = indexed.cell.character();
        }
    }
    if row[col].is_whitespace() {
        return None;
    }
    let mut start = col;
    while start > 0 && !row[start - 1].is_whitespace() {
        start -= 1;
    }
    let mut end = col;
    while end + 1 < num_cols && !row[end + 1].is_whitespace() {
        end += 1;
    }
    let token: String = row[start..=end].iter().collect();
    trim_token(&token)
}

/// Strip wrapping brackets/quotes and trailing sentence punctuation from a token,
/// keeping leading `./` and `/`.
pub(super) fn trim_token(token: &str) -> Option<String> {
    let trimmed = token
        .trim_start_matches(|c: char| "([{<\"'`".contains(c))
        .trim_end_matches(|c: char| ")]}>\"'`.,;:".contains(c));
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Drop up to two trailing `:<digits>` segments (line and column) from a path.
pub(super) fn strip_line_suffix(text: &str) -> &str {
    let mut text = text;
    for _ in 0..2 {
        match text.rsplit_once(':') {
            Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => {
                text = head;
            }
            _ => break,
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_token_drops_trailing_sentence_period() {
        assert_eq!(
            trim_token("amazon-summary-2026.html.").as_deref(),
            Some("amazon-summary-2026.html")
        );
    }

    #[test]
    fn trim_token_keeps_relative_and_absolute_prefixes() {
        assert_eq!(
            trim_token("./rel/path.rs").as_deref(),
            Some("./rel/path.rs")
        );
        assert_eq!(trim_token("/abs/path.md").as_deref(), Some("/abs/path.md"));
    }

    #[test]
    fn trim_token_strips_wrapping_delimiters_but_keeps_line_col() {
        assert_eq!(trim_token("(foo.rs:12)").as_deref(), Some("foo.rs:12"));
        assert_eq!(trim_token("\"quoted\"").as_deref(), Some("quoted"));
    }

    #[test]
    fn trim_token_all_punctuation_is_none() {
        assert_eq!(trim_token("..."), None);
        assert_eq!(trim_token(""), None);
    }

    #[test]
    fn strip_line_suffix_removes_line_and_column() {
        assert_eq!(strip_line_suffix("foo.rs:12:3"), "foo.rs");
        assert_eq!(strip_line_suffix("foo.rs:12"), "foo.rs");
        assert_eq!(strip_line_suffix("foo.rs"), "foo.rs");
        assert_eq!(strip_line_suffix("foo:bar"), "foo:bar");
    }
}
