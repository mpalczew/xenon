//! Toast state: what shows, for how long, and what ends it.

use std::time::{Duration, Instant};

use gpui::{
    Action, Context, IntoElement, ParentElement, Pixels, Render, Styled, Subscription, Task,
    Window, div,
};

use super::{Toast, paint};

pub(super) const LIFETIME: Duration = Duration::from_secs(4);

pub(super) struct Shown {
    pub(super) toast: Toast,
    pub(super) generation: u64,
    /// Time left when `since` was set; `since` is `None` while hovered.
    remaining: Duration,
    since: Option<Instant>,
}

impl Shown {
    fn left(&self) -> Duration {
        match self.since {
            Some(since) => self.remaining.saturating_sub(since.elapsed()),
            None => self.remaining,
        }
    }

    pub(super) fn progress(&self) -> f32 {
        if self.toast.sticky() {
            return 1.;
        }
        self.left().as_secs_f32() / LIFETIME.as_secs_f32()
    }
}

/// Place as a child of a `relative()` container; the island hangs from `top`,
/// centered horizontally.
pub struct ToastView {
    top: Pixels,
    shown: Option<Shown>,
    generation: u64,
    expiry: Option<Task<()>>,
    _keystrokes: Subscription,
}

impl ToastView {
    pub(super) fn new(top: Pixels, cx: &mut Context<Self>) -> Self {
        let keystrokes = cx.observe_keystrokes(|this, event, _, cx| {
            if let Some(action) = &event.action
                && this.offers(action.as_ref())
            {
                this.dismiss(cx);
            }
        });
        Self {
            top,
            shown: None,
            generation: 0,
            expiry: None,
            _keystrokes: keystrokes,
        }
    }

    pub fn show(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.shown = Some(Shown {
            toast,
            generation: self.generation,
            remaining: LIFETIME,
            // Frozen captures hold the countdown full so pixels stay deterministic.
            since: (!crate::motion_frozen(cx)).then(Instant::now),
        });
        self.schedule(cx);
        cx.notify();
    }

    /// Hides the toast. Returns whether one was showing.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) -> bool {
        self.expiry = None;
        let was_shown = self.shown.take().is_some();
        if was_shown {
            cx.notify();
        }
        was_shown
    }

    fn current(&self) -> Option<&Toast> {
        self.shown.as_ref().map(|shown| &shown.toast)
    }

    /// Whether the visible toast's action is `action`.
    pub fn offers(&self, action: &dyn Action) -> bool {
        self.current()
            .and_then(|toast| toast.action.as_ref())
            .is_some_and(|offered| offered.action.partial_eq(action))
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.expiry = None;
        let Some(shown) = &self.shown else {
            return;
        };
        if shown.toast.sticky() || shown.since.is_none() || crate::motion_frozen(cx) {
            return;
        }
        let (generation, left) = (shown.generation, shown.left());
        self.expiry = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(left).await;
            this.update(cx, |this, cx| this.expire(generation, cx)).ok();
        }));
    }

    fn expire(&mut self, generation: u64, cx: &mut Context<Self>) {
        let current = self.shown.as_ref().is_some_and(|shown| {
            shown.generation == generation && shown.since.is_some() && shown.left().is_zero()
        });
        if current {
            self.dismiss(cx);
        }
    }

    /// Hover refills the countdown and holds it; leaving restarts it.
    pub(super) fn set_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        shown.remaining = LIFETIME;
        shown.since = (!hovered).then(Instant::now);
        self.schedule(cx);
        cx.notify();
    }

    pub(super) fn run_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self
            .current()
            .and_then(|toast| toast.action.as_ref())
            .map(|offered| offered.action.boxed_clone())
        else {
            return;
        };
        self.dismiss(cx);
        window.dispatch_action(action, cx);
    }
}

impl Render for ToastView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let anchor = div()
            .absolute()
            .top(self.top)
            .left_0()
            .right_0()
            .flex()
            .justify_center();
        let Some(shown) = &self.shown else {
            return anchor;
        };
        let frozen = crate::motion_frozen(cx);
        if !shown.toast.sticky() && shown.since.is_some() && !frozen {
            window.request_animation_frame();
        }
        let island = paint::island(shown, cx);
        let animate = !frozen && !crate::reduce_motion();
        anchor.child(if animate {
            paint::arrive(island, shown.toast.kind, shown.generation)
        } else {
            island.into_any_element()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toast::ToastKind;

    fn shown(kind: ToastKind, remaining: Duration) -> Shown {
        Shown {
            toast: Toast::new(kind, "", "title"),
            generation: 1,
            remaining,
            since: None,
        }
    }

    #[test]
    fn errors_never_drain() {
        assert_eq!(
            shown(ToastKind::Error, Duration::ZERO).progress(),
            1.,
            "sticky toasts keep a full ring"
        );
    }

    #[test]
    fn paused_toast_keeps_its_time() {
        let paused = shown(ToastKind::Success, LIFETIME / 2);
        assert_eq!(paused.left(), LIFETIME / 2);
        assert!((paused.progress() - 0.5).abs() < f32::EPSILON);
    }
}
