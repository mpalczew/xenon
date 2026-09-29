//! Invariant: in the main window, keys always reach the `XenonApp` key context.
//!
//! GPUI dispatches from the focused element's node in the last rendered frame.
//! When the focused handle is dropped, or alive but not rendered, it falls back
//! to the root node: the view wrapper *above* `XenonApp`, so every app binding
//! misses. The app root's focus handle contains every surface in the window,
//! so "root contains focus" is exactly "shortcuts work".

use super::*;

impl XenonApp {
    /// Enforce the invariant twice: as soon as a frame loses focus, and, as a
    /// last resort, on any keystroke that finds focus unreachable (which also
    /// covers focus that never was reachable, so GPUI never reported a loss).
    pub(super) fn keep_window_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lost = cx.on_focus_lost(window, |this, window, cx| {
            this.ensure_keys_reach_app(window, cx);
        });
        let app = cx.weak_entity();
        let home = window.window_handle();
        let replaying = std::rc::Rc::new(std::cell::Cell::new(false));
        let stranded_key = cx.intercept_keystrokes(move |event, window, cx| {
            // A replay never repairs again, so a key can only be replayed once.
            if window.window_handle() != home || replaying.get() {
                return;
            }
            let Some(app) = app.upgrade() else { return };
            if app.update(cx, |this, cx| this.ensure_keys_reach_app(window, cx)) {
                // This key's dispatch path was fixed before the repair; replay it.
                cx.stop_propagation();
                let keystroke = event.keystroke.clone();
                let replaying = replaying.clone();
                window.defer(cx, move |window, cx| {
                    replaying.set(true);
                    window.dispatch_keystroke(keystroke, cx);
                    replaying.set(false);
                });
            }
        });
        self.services.focus_guard = vec![lost, stranded_key];
    }

    /// Moves focus back into the app when keys cannot reach it. Returns true
    /// when it had to.
    fn ensure_keys_reach_app(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.focus.contains_focused(window, cx) {
            return false;
        }
        log::info!(
            "focus unreachable ({:?}); restoring the active workspace leaf",
            window.focused(cx)
        );
        self.focus_workspace_leaf(Some(window), cx);
        // The leaf itself may not be in the rendered frame; the shell always is.
        if !self.focus.contains_focused(window, cx) {
            self.focus.focus(window, cx);
        }
        true
    }
}
