//! Terminal scrollback find: build an alacritty `Search` from a query + options.
//!
//! Backend is zed's `terminal::Search` (alacritty RegexSearch). Literals are
//! regex-escaped. Case-insensitive mode lowercases the pattern so smartcase
//! (alacritty: CI unless pattern has uppercase) stays CI.

use terminal::Search;

/// Find toggles (same labels as the editor strip).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

/// Build a `Search`, or an error string if the pattern is invalid.
pub fn build_search(query: &str, options: FindOptions) -> Result<Option<Search>, String> {
    if query.is_empty() {
        return Ok(None);
    }
    // Bare `.` matches everything in regex mode; treat as no-op (matches zed).
    if options.regex && query == "." {
        return Ok(None);
    }
    let mut pattern = if options.regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    if options.whole_word {
        pattern = format!(r"\b(?:{pattern})\b");
    }
    // Alacritty smartcase: CI unless the pattern has an uppercase letter.
    // Force CI by lowercasing when Match case is off.
    if !options.case_sensitive {
        pattern = pattern.to_lowercase();
    }
    Search::new(&pattern)
        .ok_or_else(|| "Invalid pattern".to_string())
        .map(Some)
}

/// Next/prev match index with wrap.
pub fn step_index(count: usize, current: Option<usize>, reverse: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let cur = current.unwrap_or(0);
    Some(if reverse {
        if cur == 0 { count - 1 } else { cur - 1 }
    } else {
        (cur + 1) % count
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query() {
        assert!(matches!(build_search("", FindOptions::default()), Ok(None)));
    }

    #[test]
    fn literal_builds() {
        assert!(
            build_search("error", FindOptions::default())
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn regex_meta_escaped_in_literal() {
        // `+` would be invalid/meaningful as raw regex; literal must succeed.
        assert!(
            build_search("a+", FindOptions::default())
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn bare_dot_regex_is_noop() {
        assert!(matches!(
            build_search(
                ".",
                FindOptions {
                    regex: true,
                    ..Default::default()
                }
            ),
            Ok(None)
        ));
    }

    #[test]
    fn step_wraps() {
        assert_eq!(step_index(3, Some(0), false), Some(1));
        assert_eq!(step_index(3, Some(2), false), Some(0));
        assert_eq!(step_index(3, Some(0), true), Some(2));
    }
}
