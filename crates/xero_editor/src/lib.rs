//! Editor: rope-backed buffer, edit commands, save, and external-change
//! tracking. GUI rendering and syntax highlighting layer on top of this core.

mod buffer;
mod edit;

pub use buffer::{Buffer, ExternalState, OpenError, SaveError};
pub use edit::{EditCommand, Motion};

#[cfg(test)]
mod tests;
