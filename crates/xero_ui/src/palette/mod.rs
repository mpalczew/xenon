//! Shared cmd-p-style palette shell (DESIGN elevation 2 + list selection).
//!
//! Surfaces: file finder, workspace open, run task, command/stream/help.
//! Domain ranking and confirm stay in each view; chrome, scroll, and query
//! input live here so scroll/selection fixes apply once.

mod input;
mod layout;
mod rows;

pub(crate) use input::input_registrar;
pub(crate) use layout::{
    PaletteLayout, ScrollResults, fuzzy_index_order, hint_row, hint_row_with_action,
    optional_title, panel, query_row, reveal_selected, scrim, scroll_results, step_selection,
};
pub(crate) use rows::{DetailRow, detail_row, simple_row};
