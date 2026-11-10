use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use xenon_core::WorkspaceId;
use xenon_store::{WindowGeometry, WindowState};

use crate::app::App;

const SAVE_DELAY: Duration = Duration::from_millis(400);

impl App {
    pub(crate) fn save_registry(&self) {
        if let Err(error) = xenon_store::save_registry(&self.model.borrow().registry) {
            eprintln!("[xg] save registry: {error}");
        }
    }

    pub(crate) fn save_session(&self, ws: WorkspaceId) {
        let model = self.model.borrow();
        if !model.spaces.get(&ws).is_some_and(|s| s.live) {
            return;
        }
        if let Some(state) = model.snapshot(ws)
            && let Err(error) = xenon_store::save_session(ws, &state)
        {
            eprintln!("[xg] save session {ws}: {error}");
        }
    }

    pub(crate) fn save_all(&self) {
        let ids: Vec<WorkspaceId> = self
            .model
            .borrow()
            .registry
            .workspaces
            .iter()
            .map(|r| r.id)
            .collect();
        for id in ids {
            self.save_session(id);
        }
        self.save_registry();
        self.save_sizes();
    }

    fn save_sizes(&self) {
        let (workspaces, files) = self.sidebar.sizes();
        let result = xenon_store::update_settings(|s| {
            s.workspaces_section_height = workspaces;
            s.files_section_height = files;
        });
        if let Err(error) = result {
            eprintln!("[xg] save settings: {error}");
        }
    }

    pub(crate) fn schedule_save(self: &Rc<Self>) {
        if self.save_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(SAVE_DELAY, move || {
            if let Some(app) = weak.upgrade() {
                app.save_pending.set(false);
                app.save_all();
            }
        });
    }

    pub(crate) fn save_window(&self) {
        let (w, h) = (self.win.width(), self.win.height());
        let state = if self.win.is_maximized() {
            WindowState::Maximized
        } else {
            WindowState::Windowed
        };
        let sidebar = self.split.position();
        let result = xenon_store::update_settings(|s| {
            if w > 0 && h > 0 && !self.win.is_maximized() {
                s.window = Some(WindowGeometry::new(0.0, 0.0, w as f32, h as f32, state));
            } else if let Some(g) = s.window.as_mut() {
                g.state = state;
            }
            if sidebar > 100 {
                s.sidebar_width = sidebar as f32;
            }
        });
        if let Err(error) = result {
            eprintln!("[xg] save settings: {error}");
        }
    }
}
