use std::rc::Rc;

use xenon_settings::UiSettings;

use crate::app::App;
use crate::window::{PangoProbe, mono_face};

impl App {
    pub(crate) fn change_settings(self: &Rc<Self>, change: impl FnOnce(&mut UiSettings)) {
        {
            let mut settings = self.settings.borrow_mut();
            change(&mut settings);
            settings.persist();
        }
        let probe = PangoProbe::new();
        *self.mono.borrow_mut() = mono_face(&self.settings.borrow(), &probe);
        self.apply_theme();
    }
}
