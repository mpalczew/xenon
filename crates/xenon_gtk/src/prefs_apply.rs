//! Applying a settings change to the open window: save through
//! `xenon_settings`, then restyle terminals, editors, and the CSS sheet.

use std::rc::Rc;

use gtk::prelude::WidgetExt;
use xenon_settings::{DEFAULT_FONT_SIZE, FaceFont, UiSettings};

use crate::app::App;
use crate::window::{PangoProbe, mono_face};

/// Sheet rules owned by Settings: the interface face and size as one rule on
/// the window (empty when both are the defaults, so an untouched setup keeps
/// the desktop's own look).
pub(crate) fn extra_css(ui: &FaceFont) -> String {
    let mut rules = String::new();
    if !matches!(ui.family.as_str(), ".SystemUIFont" | ".AppleSystemUIFont") {
        rules += &format!("font-family: \"{}\"; ", ui.family);
    }
    if (ui.size - DEFAULT_FONT_SIZE).abs() > 0.01 {
        rules += &format!("font-size: {:.1}%; ", ui.size / DEFAULT_FONT_SIZE * 100.0);
    }
    if rules.is_empty() {
        String::new()
    } else {
        format!("window {{ {rules}}}\n")
    }
}

impl App {
    /// Change the live settings, save them, and restyle everything.
    pub(crate) fn change_settings(self: &Rc<Self>, change: impl FnOnce(&mut UiSettings)) {
        {
            let mut settings = self.settings.borrow_mut();
            change(&mut settings);
            settings.persist();
        }
        let probe = PangoProbe(gtk::Label::new(None).pango_context());
        *self.mono.borrow_mut() = mono_face(&self.settings.borrow(), &probe);
        self.apply_theme();
    }
}
