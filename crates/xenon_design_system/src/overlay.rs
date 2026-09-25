//! Elevated overlay geometry and dismissal surface.

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FocusHandle, InteractiveElement, KeyDownEvent,
    ParentElement, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};
use theme::ThemeColors;

/// Geometry for a centered panel below the window's top edge.
#[derive(Clone, Copy)]
pub struct OverlayLayout {
    pub width: f32,
    pub max_h: f32,
    pub top: f32,
}

impl Default for OverlayLayout {
    fn default() -> Self {
        Self {
            width: 640.,
            max_h: 420.,
            top: 80.,
        }
    }
}

impl OverlayLayout {
    pub fn tall() -> Self {
        Self {
            max_h: 480.,
            top: 72.,
            ..Self::default()
        }
    }
}

/// Full-window hit target. Attach the overlay's dismiss action to this scrim.
fn overlay_scrim(id: &'static str, layout: OverlayLayout) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(layout.top))
}

/// Full-window hit target for dismissal. The panel's hitbox occludes this
/// surface, so interior clicks do not hit the dismiss target.
fn dismissible_scrim(
    id: &'static str,
    layout: OverlayLayout,
    dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    overlay_scrim(id, layout).on_click(dismiss)
}

/// Panel shell that occludes the surrounding dismiss scrim.
fn overlay_panel(layout: OverlayLayout, colors: &ThemeColors) -> Div {
    div()
        .occlude()
        .relative()
        .w(px(layout.width))
        .max_h(px(layout.max_h))
        .flex()
        .flex_col()
        .min_h_0()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .bg(colors.elevated_surface_background)
}

/// Complete keyboard and pointer shell for a transient palette. The child
/// input owns initial text focus; this shell owns dismissal and key routing.
pub struct PaletteOverlay<'a, V> {
    pub id: &'static str,
    pub layout: OverlayLayout,
    pub colors: &'a ThemeColors,
    pub focus: FocusHandle,
    pub key_context: &'static str,
    pub on_key: fn(&mut V, &KeyDownEvent, &mut Window, &mut Context<V>),
    pub on_dismiss: fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>),
    pub children: Vec<AnyElement>,
}

pub fn palette_overlay<V: 'static>(
    config: PaletteOverlay<'_, V>,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    dismissible_scrim(config.id, config.layout, cx.listener(config.on_dismiss)).child(
        overlay_panel(config.layout, config.colors)
            .track_focus(&config.focus)
            .key_context(config.key_context)
            .on_key_down(cx.listener(config.on_key))
            .children(config.children),
    )
}
