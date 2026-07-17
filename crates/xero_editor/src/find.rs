//! In-buffer find (cmd-f). Independent of vim `/` search.

use std::ops::Range;

use regex::{Regex, RegexBuilder};

/// Find toggles (VS Code-style).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

/// Result of scanning the buffer for a query.
#[derive(Clone, Debug, Default)]
pub struct FindScan {
    /// Match ranges in **char** indices (same as `Buffer::cursor`).
    pub matches: Vec<Range<usize>>,
    /// Invalid regex or empty query.
    pub error: Option<String>,
}

/// Collect all matches of `query` in `text` (full buffer string).
pub fn scan(text: &str, query: &str, options: FindOptions) -> FindScan {
    if query.is_empty() {
        return FindScan::default();
    }
    if options.regex {
        scan_regex(text, query, options)
    } else {
        scan_literal(text, query, options)
    }
}

fn scan_literal(text: &str, query: &str, options: FindOptions) -> FindScan {
    let needle = if options.case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };
    let hay = if options.case_sensitive {
        None
    } else {
        Some(text.to_lowercase())
    };
    let search_in = hay.as_deref().unwrap_or(text);
    let mut matches = Vec::new();
    let mut start = 0;
    while let Some(rel) = search_in[start..].find(&needle) {
        let byte = start + rel;
        let end_byte = byte + needle.len();
        if !options.whole_word || is_whole_word(text, byte, end_byte) {
            let start_char = text[..byte].chars().count();
            let end_char = start_char + text[byte..end_byte].chars().count();
            matches.push(start_char..end_char);
        }
        start = byte + needle.len().max(1);
        if start > search_in.len() {
            break;
        }
    }
    FindScan {
        matches,
        error: None,
    }
}

fn scan_regex(text: &str, query: &str, options: FindOptions) -> FindScan {
    let pattern = if options.whole_word {
        format!(r"\b(?:{query})\b")
    } else {
        query.to_string()
    };
    let built = RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .build();
    let re = match built {
        Ok(re) => re,
        Err(err) => {
            return FindScan {
                matches: Vec::new(),
                error: Some(err.to_string()),
            };
        }
    };
    FindScan {
        matches: regex_matches(text, &re),
        error: None,
    }
}

fn regex_matches(text: &str, re: &Regex) -> Vec<Range<usize>> {
    re.find_iter(text)
        .map(|m| {
            let start = text[..m.start()].chars().count();
            let end = start + text[m.start()..m.end()].chars().count();
            start..end
        })
        .collect()
}

fn is_whole_word(text: &str, byte_start: usize, byte_end: usize) -> bool {
    let before = text[..byte_start].chars().next_back();
    let after = text[byte_end..].chars().next();
    !is_word_char(before) && !is_word_char(after)
}

fn is_word_char(ch: Option<char>) -> bool {
    ch.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Index of the match to jump to: first at/after `cursor`, else wrap.
pub fn index_at_or_after(matches: &[Range<usize>], cursor: usize) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    matches.iter().position(|m| m.start >= cursor).or(Some(0))
}

/// Next/prev match index with wrap.
pub fn step_index(
    matches: &[Range<usize>],
    current: Option<usize>,
    reverse: bool,
) -> Option<usize> {
    let n = matches.len();
    if n == 0 {
        return None;
    }
    let cur = current.unwrap_or(0);
    Some(if reverse {
        if cur == 0 { n - 1 } else { cur - 1 }
    } else {
        (cur + 1) % n
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_case_insensitive() {
        let scan = scan("Foo foo FOO", "foo", FindOptions::default());
        assert_eq!(scan.matches.len(), 3);
    }

    #[test]
    fn literal_case_sensitive() {
        let scan = scan(
            "Foo foo FOO",
            "foo",
            FindOptions {
                case_sensitive: true,
                ..Default::default()
            },
        );
        assert_eq!(scan.matches.len(), 1);
        assert_eq!(scan.matches[0], 4..7);
    }

    #[test]
    fn whole_word() {
        let scan = scan(
            "cat catalog cat",
            "cat",
            FindOptions {
                whole_word: true,
                ..Default::default()
            },
        );
        assert_eq!(scan.matches.len(), 2);
        assert_eq!(scan.matches[0], 0..3);
        assert_eq!(scan.matches[1], 12..15);
    }

    #[test]
    fn regex_digits() {
        let scan = scan(
            "a1 b22 c",
            r"\d+",
            FindOptions {
                regex: true,
                ..Default::default()
            },
        );
        assert_eq!(scan.matches.len(), 2);
        assert_eq!(scan.matches[0], 1..2);
        assert_eq!(scan.matches[1], 4..6);
    }

    #[test]
    fn bad_regex() {
        let scan = scan(
            "abc",
            "(",
            FindOptions {
                regex: true,
                ..Default::default()
            },
        );
        assert!(scan.error.is_some());
        assert!(scan.matches.is_empty());
    }

    #[test]
    fn unicode_char_indices() {
        let scan = scan("a日b日c", "日", FindOptions::default());
        assert_eq!(scan.matches.len(), 2);
        assert_eq!(scan.matches[0], 1..2);
        assert_eq!(scan.matches[1], 3..4);
    }

    #[test]
    fn step_wraps() {
        let matches = vec![0..1, 5..6, 10..11];
        assert_eq!(step_index(&matches, Some(0), false), Some(1));
        assert_eq!(step_index(&matches, Some(2), false), Some(0));
        assert_eq!(step_index(&matches, Some(0), true), Some(2));
    }

    #[test]
    fn index_at_or_after_cursor() {
        let matches = vec![2..3, 8..9];
        assert_eq!(index_at_or_after(&matches, 0), Some(0));
        assert_eq!(index_at_or_after(&matches, 5), Some(1));
        assert_eq!(index_at_or_after(&matches, 20), Some(0));
    }
}
