//! Shared-secret token checks.

use uuid::Uuid;

/// Generate a new remote auth token.
pub fn new_token() -> String {
    Uuid::new_v4().to_string()
}

/// Constant-time-ish equality for short tokens (not crypto-grade; enough for LAN).
pub fn token_ok(expected: &str, presented: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    if expected.len() != presented.len() {
        return false;
    }
    expected
        .as_bytes()
        .iter()
        .zip(presented.as_bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_expected() {
        assert!(!token_ok("", "anything"));
    }

    #[test]
    fn accepts_exact_match() {
        assert!(token_ok("abc", "abc"));
    }

    #[test]
    fn rejects_mismatch() {
        assert!(!token_ok("abc", "abd"));
        assert!(!token_ok("abc", "ab"));
    }
}
