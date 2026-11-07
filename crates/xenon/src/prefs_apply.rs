//! Applying a settings change to the open window: save through
//! `xenon_settings`, then restyle terminals, editors, and the CSS sheet.

use std::rc::Rc;

use gtk::prelude::WidgetExt;
use xenon_settings::UiSettings;

use crate::app::App;
use crate::window::{PangoProbe, mono_face};

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
