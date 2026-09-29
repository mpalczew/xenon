//! Device tokens: generation, hashing, comparison.

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A fresh 256-bit-ish device token (two v4 UUIDs, hex). Shown to the phone once.
pub fn new_device_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// SHA-256 hex digest; the only form of a device token that is stored.
pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Constant-time equality for equal-length secrets.
pub fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_and_distinct() {
        let a = new_device_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, new_device_token());
    }

    #[test]
    fn hash_is_stable_hex() {
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
