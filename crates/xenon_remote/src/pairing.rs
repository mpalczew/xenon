//! One-time pairing codes shown on the Mac.

use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::auth::ct_eq;

/// How long a pairing code stays valid after it is shown on the Mac.
pub const PAIRING_TTL: Duration = Duration::from_secs(10 * 60);

/// One-time code shown on the Mac: 6 digits to type, or a secret in the QR link.
#[derive(Clone, Debug)]
pub struct PairingCode {
    pub digits: String,
    pub secret: String,
    pub expires_at: Instant,
}

impl PairingCode {
    pub fn issue(now: Instant) -> Self {
        let n = Uuid::new_v4().as_u128() % 1_000_000;
        Self {
            digits: format!("{n:06}"),
            secret: Uuid::new_v4().simple().to_string(),
            expires_at: now + PAIRING_TTL,
        }
    }

    /// `123 456` for display.
    pub fn display_digits(&self) -> String {
        format!("{} {}", &self.digits[..3], &self.digits[3..])
    }

    pub fn is_live(&self, now: Instant) -> bool {
        now < self.expires_at
    }

    /// Accepts the digits (spaces ignored) or the QR secret.
    pub fn matches(&self, presented: &str, now: Instant) -> bool {
        if !self.is_live(now) {
            return false;
        }
        let compact: String = presented.chars().filter(|c| !c.is_whitespace()).collect();
        ct_eq(&self.digits, &compact) || ct_eq(&self.secret, &compact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_accepts_digits_or_secret_until_expiry() {
        let now = Instant::now();
        let code = PairingCode::issue(now);
        assert_eq!(code.digits.len(), 6);
        assert!(code.matches(&code.display_digits(), now));
        assert!(code.matches(&code.secret, now));
        assert!(!code.matches("000000x", now));
        assert!(!code.matches(&code.digits, now + PAIRING_TTL));
    }
}
