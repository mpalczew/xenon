//! Applying a settings change to the open window: save through
//! `xenon_settings`, then restyle terminals, editors, and the CSS sheet. Also
//! the theme preview that the picker in Settings uses before a theme is kept.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::WidgetExt;
use xenon_settings::{DEFAULT_FONT_SIZE, FaceFont, UiSettings};

use crate::app::App;
use crate::theme::Palette;
use crate::window::{PangoProbe, mono_face};

thread_local! {
    /// Theme name and whether it is dark, while the picker is trying one.
    static PREVIEW: RefCell<Option<(String, bool)>> = const { RefCell::new(None) };
}

pub(crate) fn previewing() -> Option<(String, bool)> {
    PREVIEW.with(|p| p.borrow().clone())
}

/// Sheet rules owned by Settings: the interface face and size as one rule on
/// the window (empty when both are the defaults, so an untouched setup keeps
/// the desktop's own look) and the size and mark of the theme tiles.
pub(crate) fn extra_css(ui: &FaceFont, palette: &Palette) -> String {
    let mut rules = String::new();
    if !matches!(ui.family.as_str(), ".SystemUIFont" | ".AppleSystemUIFont") {
        rules += &format!("font-family: \"{}\"; ", ui.family);
    }
    if (ui.size - DEFAULT_FONT_SIZE).abs() > 0.01 {
        rules += &format!("font-size: {:.1}%; ", ui.size / DEFAULT_FONT_SIZE * 100.0);
    }
    let window = if rules.is_empty() {
        String::new()
    } else {
        format!("window {{ {rules}}}\n")
    };
    let accent = palette.color("text.accent", "#74ade8");
    format!(
        "{window}button.xtile {{ min-width: 56px; min-height: 34px; padding: 0 8px; border-radius: 8px; }}\n\
         button.xtile.current {{ box-shadow: 0 0 0 2px {accent}; }}\n"
    )
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

    /// Show a theme without saving it.
    pub(crate) fn preview_theme(&self, name: &str, dark: bool) {
        PREVIEW.with(|p| *p.borrow_mut() = Some((name.to_string(), dark)));
        self.apply_theme();
    }

    /// Go back to the saved theme.
    pub(crate) fn end_preview(&self) {
        if PREVIEW.with(|p| p.borrow_mut().take()).is_some() {
            self.apply_theme();
        }
    }

    /// Keep a theme in its light or dark slot. The mode does not change.
    pub(crate) fn keep_theme(self: &Rc<Self>, name: &str, dark: bool) {
        PREVIEW.with(|p| *p.borrow_mut() = None);
        self.change_settings(|s| s.set_theme_name(dark, name));
    }
}
