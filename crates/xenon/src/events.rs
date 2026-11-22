use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use vte::prelude::*;
use xenon_core::{PaneId, TabId, TabState, WorkspaceId};

use crate::app::App;
use crate::model::TabKind;

const PULSE_TICK: Duration = Duration::from_millis(100);
const PULSE_PERIOD_S: f64 = 1.4;
const PULSE_LEVELS: i32 = 5;

impl App {
    pub(crate) fn relabel(&self, ws: WorkspaceId, tab: TabId) {
        let located = {
            let model = self.model.borrow();
            let content = model.spaces.get(&ws).map(|s| &s.content);
            content
                .and_then(|c| c.find_tab(tab))
                .and_then(|(pane, index)| model.tab(ws, tab).map(|t| (pane, t.label(index))))
        };
        if let Some((pane, label)) = located
            && let Some(view) = self
                .slots
                .borrow()
                .get(&ws)
                .and_then(|s| s.panes.get(&pane).cloned())
        {
            view.set_title(tab, &label);
        }
        {
            let model = self.model.borrow();
            if let Some(ws) = model.active() {
                self.title
                    .set_subtitle(&crate::app::space_label(&model, ws));
            }
        }
        self.refresh_save_button();
    }

    pub(crate) fn refresh_save_button(&self) {
        let dirty = self.model.borrow().active_editor_dirty();
        self.save_button.set_visible(dirty);
    }

    pub(crate) fn save_active_editor(&self) {
        if let Some(editor) = self.current_editor() {
            editor.save();
        }
    }

    pub(crate) fn save_active_editor_as(&self) {
        let key = self.model.borrow().active_editor_key();
        if let Some((ws, tab)) = key
            && let Some(editor) = self.current_editor()
        {
            editor.save_as(ws, tab);
        }
    }

    /// The editor saved under a new name: its tab now points there. The body is rebuilt
    /// from the new file, like a renamed one.
    pub(crate) fn editor_saved_as(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, path: &Path) {
        if let Some(t) = self.model.borrow_mut().tab_mut(ws, tab)
            && let TabKind::Kept(TabState::Editor { path: old, .. }) = &mut t.kind
        {
            *old = path.to_path_buf();
            t.modified = false;
        }
        let key = (ws, tab);
        self.bodies.borrow_mut().remove(&key);
        self.editors.borrow_mut().remove(&key);
        self.touch_recent(ws, path);
        self.changed(ws);
        self.refresh_save_button();
    }

    pub(crate) fn editor_modified(&self, ws: WorkspaceId, tab: TabId, modified: bool) {
        if let Some(t) = self.model.borrow_mut().tab_mut(ws, tab) {
            t.modified = modified;
        }
        self.relabel(ws, tab);
    }

    pub(crate) fn pane_focused(self: &Rc<Self>, ws: WorkspaceId, pane: PaneId) {
        if self.tab_sync.get() || self.launcher.restoring.get() {
            return;
        }
        let changed = self.model.borrow_mut().focus_pane(ws, pane);
        let dismissed = self.dismiss_viewed();
        if changed || dismissed {
            self.sync(ws);
        }
    }

    pub(crate) fn term_exited(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, code: i32) {
        if self.tab_sync.get() {
            return;
        }
        let view = self.term_view(ws, tab);
        if let Some(view) = view {
            view.term.feed(
                format!("\r\n\x1b[2m[process exited with code {code}]\x1b[0m\r\n").as_bytes(),
            );
        }
        self.sync(ws);
        if self.tasks.owns(ws, tab) {
            self.task_finished(ws, tab);
            return;
        }
        let delay = self.settings.borrow().terminal_auto_close.delay();
        if let Some(delay) = delay {
            let weak = Rc::downgrade(self);
            glib::timeout_add_local_once(delay, move || {
                if let Some(app) = weak.upgrade() {
                    app.close_exited(ws, tab);
                }
            });
        }
    }

    fn close_exited(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let exited = self
            .model
            .borrow()
            .tab(ws, tab)
            .is_some_and(|t| t.exited.is_some());
        if exited {
            self.close_tab_ui(ws, tab);
        }
    }

    pub(crate) fn ensure_pulse(self: &Rc<Self>) {
        let working = self.model.borrow().any_working();
        let running = self.pulse_source.borrow().is_some();
        if !working || running {
            return;
        }
        let weak = Rc::downgrade(self);
        let source = glib::timeout_add_local(PULSE_TICK, move || {
            let Some(app) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            app.pulse_tick()
        });
        *self.pulse_source.borrow_mut() = Some(source);
    }

    fn pulse_tick(self: &Rc<Self>) -> glib::ControlFlow {
        if !self.model.borrow().any_working() {
            return self.stop_pulse();
        }
        let t = self.started.elapsed().as_secs_f64();
        let wave = 0.5 + 0.5 * (t * std::f64::consts::TAU / PULSE_PERIOD_S).cos();
        let level = (wave * (PULSE_LEVELS - 1) as f64).round() as i32;
        if self.pulse_level.replace(level) == level {
            return glib::ControlFlow::Continue;
        }
        let opacity = 0.3 + 0.7 * f64::from(level) / (PULSE_LEVELS - 1) as f64;
        self.sidebar.pulse(opacity);
        glib::ControlFlow::Continue
    }

    fn stop_pulse(self: &Rc<Self>) -> glib::ControlFlow {
        *self.pulse_source.borrow_mut() = None;
        self.paint_all();
        glib::ControlFlow::Break
    }
}

impl App {
    pub(crate) fn open_editor(self: &Rc<Self>, ws: WorkspaceId, path: &std::path::Path) {
        if crate::image::is_image(path) {
        } else if let Err(why) = crate::editor::check(path) {
            self.toast(&format!("Opening {} outside Xenon: {why}", path.display()));
            self.launch(&gio::File::for_path(path).uri());
            return;
        }
        if self.model.borrow().active() != Some(ws) {
            self.activate_workspace(ws);
        }
        if self.model.borrow_mut().open_editor(ws, path).is_some() {
            self.touch_recent(ws, path);
            self.changed(ws);
        }
    }
}
