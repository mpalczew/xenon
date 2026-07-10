//! Buffer search for `/`, `?`, `n`, `N`, `*`.

use ropey::Rope;

#[derive(Clone, Debug)]
pub struct SearchState {
    pub pattern: String,
    pub forward: bool,
}

impl SearchState {
    pub fn new(pattern: String, forward: bool) -> Self {
        Self { pattern, forward }
    }
}

/// Find next match starting after `from` (or before if not forward). Returns start index.
pub fn find(rope: &Rope, pattern: &str, from: usize, forward: bool) -> Option<usize> {
    if pattern.is_empty() {
        return None;
    }
    let text = rope.to_string();
    if forward {
        let start = (from + 1).min(text.len());
        if let Some(rel) = text[start..].find(pattern) {
            return Some(char_index(&text, start + rel));
        }
        // Wrap.
        text[..from.min(text.len())]
            .find(pattern)
            .map(|byte| char_index(&text, byte))
    } else {
        let end = from.min(text.len());
        if let Some(byte) = text[..end].rfind(pattern) {
            return Some(char_index(&text, byte));
        }
        // Wrap.
        text[from.min(text.len())..]
            .rfind(pattern)
            .map(|rel| char_index(&text, from.min(text.len()) + rel))
    }
}

fn char_index(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_next() {
        let rope = Rope::from_str("ab x ab");
        assert_eq!(find(&rope, "ab", 0, true), Some(5));
    }
}
