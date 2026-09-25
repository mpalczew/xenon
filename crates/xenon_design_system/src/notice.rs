//! A dismissible notice whose lifetime is owned by the control.

use std::time::Duration;

use gpui::{Context, Div, ParentElement, Styled, Task, div, px};
use theme::ThemeColors;

pub fn notice_panel(message: impl Into<String>, colors: &ThemeColors) -> Div {
    div()
        .absolute()
        .bottom(px(18.))
        .right(px(18.))
        .px_3()
        .py_2()
        .rounded_sm()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
        .flex()
        .items_center()
        .gap_3()
        .child(message.into())
}

#[derive(Default)]
pub struct TimedNotice {
    message: Option<String>,
    generation: u64,
    expiry: Option<Task<()>>,
}

impl TimedNotice {
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn show<V: 'static>(
        &mut self,
        message: impl Into<String>,
        cx: &mut Context<V>,
        expire: fn(&mut V, u64, &mut Context<V>),
    ) {
        self.message = Some(message.into());
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.expiry = Some(cx.spawn(async move |entity, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            entity
                .update(cx, |owner, cx| expire(owner, generation, cx))
                .ok();
        }));
        cx.notify();
    }

    pub fn expire(&mut self, generation: u64) -> bool {
        if self.generation != generation {
            return false;
        }
        self.message = None;
        self.expiry = None;
        true
    }

    pub fn dismiss(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.message = None;
        self.expiry = None;
    }
}

#[cfg(test)]
mod tests {
    use super::TimedNotice;

    #[test]
    fn older_expiration_cannot_clear_newer_message() {
        let mut notice = TimedNotice {
            generation: 1,
            message: Some("new".into()),
            ..Default::default()
        };
        assert!(!notice.expire(0));
        assert_eq!(notice.message(), Some("new"));
    }
}
