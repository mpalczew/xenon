//! Shared caret cadence for focused native text surfaces.

use std::time::Duration;

use gpui::{Context, Task};

use crate::motion_frozen;

const HALF_PERIOD: Duration = Duration::from_millis(600);

pub struct CursorBlink {
    visible: bool,
    active: bool,
    generation: u64,
    timer: Option<Task<()>>,
}

impl Default for CursorBlink {
    fn default() -> Self {
        Self {
            visible: true,
            active: false,
            generation: 0,
            timer: None,
        }
    }
}

impl CursorBlink {
    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn update_focus<V: 'static>(
        &mut self,
        focused: bool,
        cx: &mut Context<V>,
        tick: fn(&mut V, u64, &mut Context<V>) -> bool,
    ) {
        if motion_frozen(cx) {
            self.visible = true;
            self.active = false;
            self.timer = None;
            return;
        }
        if self.active == focused {
            return;
        }
        self.active = focused;
        if focused {
            self.reset(cx, tick);
        } else {
            self.generation = self.generation.wrapping_add(1);
            self.timer = None;
            self.visible = false;
        }
    }

    pub fn reset<V: 'static>(
        &mut self,
        cx: &mut Context<V>,
        tick: fn(&mut V, u64, &mut Context<V>) -> bool,
    ) {
        self.visible = true;
        self.generation = self.generation.wrapping_add(1);
        if !self.active || motion_frozen(cx) {
            return;
        }
        let generation = self.generation;
        self.timer = Some(cx.spawn(async move |owner, cx| {
            loop {
                cx.background_executor().timer(HALF_PERIOD).await;
                let Ok(keep_running) = owner.update(cx, |view, cx| tick(view, generation, cx))
                else {
                    break;
                };
                if !keep_running {
                    break;
                }
            }
        }));
        cx.notify();
    }

    pub fn tick(&mut self, generation: u64) -> bool {
        if !self.active || self.generation != generation {
            return false;
        }
        self.visible = !self.visible;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::CursorBlink;

    #[test]
    fn stale_ticks_do_not_hide_a_reset_caret() {
        let mut blink = CursorBlink {
            active: true,
            generation: 2,
            ..Default::default()
        };
        assert!(!blink.tick(1));
        assert!(blink.visible());
        assert!(blink.tick(2));
        assert!(!blink.visible());
    }
}
