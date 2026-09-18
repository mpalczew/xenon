//! Dock trampoline: adopt the last A/B slot, then exec `xenon-bin`.
//!
//! `CFBundleExecutable` is this crate’s binary. The GUI lives beside it as
//! [`GUI_BINARY`]. Already-set `XENON_DATA_DIR` / `XERO_DATA_DIR` win.

mod launch;
mod slot;

pub use launch::{GUI_BINARY, exec_gui, gui_binary};
pub use slot::{SlotLaunch, apply_slot_env, resolve_slot_launch};
