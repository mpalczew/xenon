use std::rc::Rc;

use adw::prelude::*;
use xenon_settings::{Surface, UiSettings};

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

impl App {
    pub(crate) fn zoom(self: &Rc<Self>, step: Option<f32>) {
        let editor = self.current_editor();
        let diff = {
            let model = self.model.borrow();
            let ws = model.active();
            ws.and_then(|w| model.spaces.get(&w))
                .and_then(|s| s.content.active_tab())
                .is_some_and(|t| matches!(t.kind, crate::model::TabKind::Diff { .. }))
        };
        if editor
            .as_ref()
            .is_some_and(|e| e.view.has_css_class("xprose"))
        {
            self.change_settings(|s| s.zoom_prose(step));
        } else if editor.is_some() || diff {
            self.change_settings(|s| s.zoom(Surface::Editor, step));
        } else {
            {
                let mut settings = self.settings.borrow_mut();
                settings.zoom(Surface::Terminal, step);
                settings.persist();
            }
            for view in self.terms.borrow().values() {
                self.restyle_term(view);
            }
        }
    }
}
