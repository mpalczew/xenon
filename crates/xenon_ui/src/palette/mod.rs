//! Shared cmd-p-style palette shell (DESIGN elevation 2 + list selection).
//!
//! Surfaces: file finder, workspace open, run task, command/stream/help.
//! Domain ranking and confirm stay in each view; chrome, scroll, and query
//! input live here so scroll/selection fixes apply once.

mod layout;

pub(crate) use layout::{
    PaletteLayout, ScrollResults, fuzzy_index_order, match_hits, reveal_selected, scroll_results,
    step_selection,
};
