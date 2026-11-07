//! Terminal signals in, status out. Each handler passes a raw signal to the
//! model and repaints only when the model says something visible changed.
//! The pulse runs only while something is working.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib;
use vte::prelude::*;
use xenon_core::{IDLE_AFTER, TabId, WorkspaceId};

use crate::app::App;

/// The pulse redraws at most this often.
const PULSE_TICK: Duration = Duration::from_millis(100);
/// One full fade in and out.
const PULSE_PERIOD_S: f64 = 1.4;
/// Distinct opacities in a fade.
const PULSE_LEVELS: i32 = 5;

impl App {
    pub(crate) fn term_output(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let became_working = self.model.borrow_mut().on_output(ws, tab);
        let view = self.terms.borrow().get(&(ws, tab)).cloned();
        if let Some(view) = view {
            view.last_output.set(Instant::now());
            if !view.quiet_armed.replace(true) {
                self.arm_quiet(ws, tab, IDLE_AFTER);
            }
        }
        if became_working {
            self.paint_all();
        }
    }

    /// One timer per burst, not per output batch: when it fires it either
    /// settles the terminal or sleeps for the time still left.
    fn arm_quiet(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, delay: Duration) {
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(delay, move || {
            if let Some(app) = weak.upgrade() {
                app.quiet_check(ws, tab);
            }
        });
    }

    fn quiet_check(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let Some(view) = self.terms.borrow().get(&(ws, tab)).cloned() else {
            return;
        };
        let elapsed = view.last_output.get().elapsed();
        if elapsed < IDLE_AFTER {
            self.arm_quiet(ws, tab, IDLE_AFTER - elapsed);
            return;
        }
        view.quiet_armed.set(false);
        if self.model.borrow_mut().on_quiet(ws, tab) {
            self.paint_all();
        }
    }

    pub(crate) fn term_bell(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        if self.model.borrow_mut().on_bell(ws, tab) {
            self.paint_all();
        }
    }

    pub(crate) fn term_interaction(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        if self.model.borrow_mut().on_interaction(ws, tab) {
            self.paint_all();
        }
    }

    pub(crate) fn term_title(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, raw: &str) {
        let Some(view) = self.terms.borrow().get(&(ws, tab)).cloned() else {
            return;
        };
        if !self
            .model
            .borrow_mut()
            .set_title(ws, tab, raw, &view.spawn_dir)
        {
            return;
        }
        self.publish_state();
        self.relabel(ws, tab);
    }

    /// Only the chip text changes; a full sync would rebuild every chip.
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
        let model = self.model.borrow();
        if let Some(ws) = model.active() {
            self.title
                .set_subtitle(&crate::app::space_label(&model, ws));
        }
    }

    /// An editor's unsaved state changed.
    pub(crate) fn editor_modified(&self, ws: WorkspaceId, tab: TabId, modified: bool) {
        if let Some(t) = self.model.borrow_mut().tab_mut(ws, tab) {
            t.modified = modified;
        }
        self.relabel(ws, tab);
    }

    /// A terminal took keyboard focus (click or key): the model's focused pane
    /// and active tab follow it, so commands target what the user is using.
    pub(crate) fn term_focused(self: &Rc<Self>, ws: WorkspaceId, tab: TabId) {
        let changed = {
            let mut model = self.model.borrow_mut();
            let changed = model.select_tab(ws, tab);
            let dismissed = model.dismiss_viewed();
            changed || dismissed
        };
        if changed {
            self.sync(ws);
        }
    }

    pub(crate) fn term_exited(self: &Rc<Self>, ws: WorkspaceId, tab: TabId, code: i32) {
        self.model.borrow_mut().on_exit(ws, tab, code);
        let view = self.terms.borrow().get(&(ws, tab)).cloned();
        if let Some(view) = view {
            view.term.feed(
                format!("\r\n\x1b[2m[process exited with code {code}]\x1b[0m\r\n").as_bytes(),
            );
        }
        self.sync(ws);
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

    /// Start the pulse timer if something is working and it is not running.
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
        let (active, working) = {
            let model = self.model.borrow();
            let Some(ws) = model.active() else {
                return self.stop_pulse();
            };
            let mut tabs: Vec<TabId> = Vec::new();
            if let Some(root) = model.spaces.get(&ws).and_then(|s| s.content.root.as_ref()) {
                root.for_each_leaf(&mut |leaf| {
                    tabs.extend(
                        leaf.tabs
                            .iter()
                            .map(|t| t.id)
                            .filter(|t| model.is_working(ws, *t)),
                    );
                });
            }
            if !model.any_working() {
                return self.stop_pulse();
            }
            (ws, tabs)
        };
        // Five steps per fade: the timer wakes at 10 Hz but a frame is drawn
        // only when the step changes (about 6 per second), which is what the
        // idle budget pays for.
        let t = self.started.elapsed().as_secs_f64();
        let wave = 0.5 + 0.5 * (t * std::f64::consts::TAU / PULSE_PERIOD_S).cos();
        let level = (wave * (PULSE_LEVELS - 1) as f64).round() as i32;
        if self.pulse_level.replace(level) == level {
            return glib::ControlFlow::Continue;
        }
        let opacity = 0.3 + 0.7 * f64::from(level) / (PULSE_LEVELS - 1) as f64;
        if let Some(slot) = self.slots.borrow().get(&active) {
            for pane in slot.panes.values() {
                pane.pulse(&working, opacity);
            }
        }
        self.sidebar.pulse(opacity);
        glib::ControlFlow::Continue
    }

    /// Drop the timer source and repaint so rails return to full strength.
    fn stop_pulse(self: &Rc<Self>) -> glib::ControlFlow {
        *self.pulse_source.borrow_mut() = None;
        self.paint_all();
        glib::ControlFlow::Break
    }
}
