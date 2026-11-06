//! Looping ambient pulse that redraws at a low tick rate instead of every vsync.
//!
//! GPUI's repeating `with_animation` asks for the next frame forever, and each frame
//! repaints the whole window. On a 120 Hz 5K panel that held a CPU core at ~50% while
//! any agent was working. A slow opacity pulse on a 2px line needs ~10 steps a second.

use std::{
    cell::RefCell,
    collections::HashSet,
    sync::OnceLock,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Element, ElementId, EntityId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};

/// Redraw interval while a pulse is on screen.
const TICK: Duration = Duration::from_millis(100);

/// Shared clock so every pulse on screen breathes in step.
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

thread_local! {
    /// Views with a tick already scheduled; one timer per view, however many pulses it draws.
    static PENDING: RefCell<HashSet<EntityId>> = RefCell::new(HashSet::new());
}

/// Wrap `element` in a repeating pulse. `easing` maps the cycle position (0..1) to the
/// value handed to `animator`.
pub fn slow_pulse<E: IntoElement + 'static>(
    element: E,
    period: Duration,
    easing: impl Fn(f32) -> f32 + 'static,
    animator: impl Fn(E, f32) -> E + 'static,
) -> SlowPulse<E> {
    SlowPulse {
        element: Some(element),
        period,
        easing: Box::new(easing),
        animator: Box::new(animator),
    }
}

/// The agent-working breath shared by the sidebar line, tab rail and overflow pip:
/// opacity 0.5..1.0 over 1.4 s.
pub fn working_pulse<E: IntoElement + 'static>(
    element: E,
    animator: impl Fn(E, f32) -> E + 'static,
) -> SlowPulse<E> {
    slow_pulse(
        element,
        Duration::from_millis(1400),
        |t| (t * std::f32::consts::TAU).sin().mul_add(0.25, 0.75),
        animator,
    )
}

pub struct SlowPulse<E> {
    element: Option<E>,
    period: Duration,
    easing: Box<dyn Fn(f32) -> f32>,
    animator: Box<dyn Fn(E, f32) -> E>,
}

impl<E: IntoElement + 'static> IntoElement for SlowPulse<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E: IntoElement + 'static> Element for SlowPulse<E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let period = self.period.as_secs_f32().max(f32::EPSILON);
        let phase = (epoch().elapsed().as_secs_f32() / period).fract();
        let value = (self.easing)(phase);
        let element = self.element.take().expect("request_layout runs once");
        let mut element = (self.animator)(element, value).into_any_element();
        schedule_tick(window, cx);
        (element.request_layout(window, cx), element)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<Pixels>,
        element: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<Pixels>,
        element: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

/// Re-render the current view once after `TICK`. A pulse that is no longer drawn
/// simply stops scheduling, so the window goes quiet.
fn schedule_tick(window: &mut Window, cx: &mut App) {
    let view = window.current_view();
    if !PENDING.with_borrow_mut(|pending| pending.insert(view)) {
        return;
    }
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(TICK).await;
            PENDING.with_borrow_mut(|pending| pending.remove(&view));
            cx.update(|_, cx| cx.notify(view)).ok();
        })
        .detach();
}
