//! Editable Markdown "prose" mode: a projection of the one source buffer.
//! Model parses blocks, display maps source to shown text, layout shapes it.

pub mod display;
pub mod edit;
pub mod layout;
pub mod model;
pub mod paint;
pub mod projection;
pub mod style;
