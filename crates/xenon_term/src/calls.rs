//! Fire-and-forget calls: no answer comes back.

use anyhow::Result;

use crate::wire::Request;
use crate::{Client, Size, TermId};

impl Client {
    pub fn input(&self, id: TermId, bytes: &[u8]) -> Result<()> {
        self.send(
            0,
            &Request::Input {
                id,
                bytes: bytes.to_vec(),
            },
        )
    }

    pub fn resize(&self, id: TermId, size: Size) -> Result<()> {
        self.send(0, &Request::Resize { id, size })
    }

    /// Hang up the child. Idempotent.
    pub fn close(&self, id: TermId) -> Result<()> {
        self.send(0, &Request::Close { id })
    }
}
