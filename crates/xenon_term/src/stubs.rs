//! Client calls the spike does not need yet.

use anyhow::{Result, bail};

use crate::{Client, Size, TermId};

impl Client {
    /// Not implemented in the spike: size claims from phones.
    pub fn claim(&self, _id: TermId, _size: Size) -> Result<()> {
        bail!("claim: not implemented")
    }

    /// Not implemented in the spike: attention state.
    pub fn seen(&self, _id: TermId) -> Result<()> {
        bail!("seen: not implemented")
    }
}
