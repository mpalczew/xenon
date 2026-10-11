//! Navigation keys a palette field does not edit.
//!
//! `ParentNavigation` stops the key, so the overlay `on_key_down` never sees
//! Enter, Escape, Tab, Up, or Down. Callers match [`PaletteInput::Navigate`].

use crate::TextInputEvent;

pub enum PaletteInput {
    Query(String),
    Navigate {
        key: String,
        shift: bool,
        platform: bool,
    },
    Ignore,
}

pub fn palette_input(event: &TextInputEvent) -> PaletteInput {
    match event {
        TextInputEvent::Changed(query) => PaletteInput::Query(query.clone()),
        TextInputEvent::ParentKey {
            key,
            shift,
            platform,
        } => PaletteInput::Navigate {
            key: key.clone(),
            shift: *shift,
            platform: *platform,
        },
        TextInputEvent::Submit(_) | TextInputEvent::Cancel => PaletteInput::Ignore,
    }
}
