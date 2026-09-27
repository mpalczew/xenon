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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigate_is_not_a_query_change() {
        let event = TextInputEvent::ParentKey {
            key: "down".into(),
            shift: false,
            platform: true,
        };
        match palette_input(&event) {
            PaletteInput::Navigate {
                key,
                shift,
                platform,
            } => {
                assert_eq!(key, "down");
                assert!(!shift);
                assert!(platform);
            }
            PaletteInput::Query(_) | PaletteInput::Ignore => panic!("down must navigate"),
        }
    }

    #[test]
    fn submit_does_not_double_as_navigate() {
        let event = TextInputEvent::Submit("xenon".into());
        assert!(matches!(palette_input(&event), PaletteInput::Ignore));
    }
}
