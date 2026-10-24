//! Selectable styled text block for markdown preview (hit-test + selection paint).

use std::ops::Range;

use gpui::{
    App, Bounds, CursorStyle, DispatchPhase, Element, ElementId, Entity, GlobalElementId,
    HighlightStyle, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, SharedString, StyledText, Window,
};

use super::select::highlights_with_selection;
use super::state::{PreviewClick, PreviewState};

/// Clickable ranges in a block's text and their destinations.
pub type Links = Vec<(Range<usize>, SharedString)>;

fn link_at(links: &Links, offset: usize) -> Option<SharedString> {
    links
        .iter()
        .find(|(range, _)| range.contains(&offset))
        .map(|(_, dest)| dest.clone())
}

/// One selectable text run in the preview, keyed by document-order block index.
pub struct SelectableBlock {
    id: ElementId,
    block_ix: usize,
    text: SharedString,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    links: Links,
    host: Entity<PreviewState>,
    selection_color: Hsla,
}

impl SelectableBlock {
    pub fn new(
        block_ix: usize,
        text: SharedString,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
        host: Entity<PreviewState>,
        selection_color: Hsla,
    ) -> Self {
        Self {
            id: ElementId::named_usize("md-sel", block_ix),
            block_ix,
            text,
            highlights,
            links: Vec::new(),
            host,
            selection_color,
        }
    }

    /// Clickable ranges (links, path-like code spans) in `text`.
    pub fn with_links(mut self, links: Links) -> Self {
        self.links = links;
        self
    }
}

impl IntoElement for SelectableBlock {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableBlock {
    type RequestLayoutState = StyledText;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let sel_range = {
            let host = self.host.read(cx);
            host.sel().local_range(self.block_ix, self.text.len())
        };
        let highlights = highlights_with_selection(
            self.highlights.clone(),
            sel_range,
            self.selection_color,
            self.text.len(),
        );
        let mut styled = StyledText::new(self.text.clone()).with_highlights(highlights);
        let (layout_id, ()) = Element::request_layout(&mut styled, None, inspector_id, window, cx);
        (layout_id, styled)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<gpui::Pixels>,
        styled: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        Element::prepaint(styled, None, inspector_id, bounds, &mut (), window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<gpui::Pixels>,
        styled: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let text_layout = styled.layout().clone();
        let host = self.host.clone();
        let block_ix = self.block_ix;
        let pending = host.read(cx).sel().pending;

        if hitbox.is_hovered(window) {
            let on_link = !pending
                && text_layout
                    .index_for_position(window.mouse_position())
                    .ok()
                    .and_then(|offset| link_at(&self.links, offset))
                    .is_some();
            let cursor = if on_link {
                CursorStyle::PointingHand
            } else {
                CursorStyle::IBeam
            };
            window.set_cursor_style(cursor, hitbox);
        }

        {
            let host = host.clone();
            let text_layout = text_layout.clone();
            let hitbox = hitbox.clone();
            let links = std::mem::take(&mut self.links);
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                    return;
                }
                if !hitbox.is_hovered(window) {
                    return;
                }
                let hit = text_layout.index_for_position(event.position);
                let link = hit.ok().and_then(|offset| link_at(&links, offset));
                let offset = hit.unwrap_or_else(|e| e);
                let shift = event.modifiers.shift;
                let clicks = event.click_count.max(1);
                host.update(cx, |state, cx| {
                    state.mouse_down(
                        PreviewClick {
                            block: block_ix,
                            offset,
                            count: clicks,
                            shift,
                            link,
                        },
                        cx,
                    );
                });
                window.prevent_default();
            });
        }

        {
            let host = host.clone();
            let text_layout = text_layout.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                if !host.read(cx).sel().pending {
                    return;
                }
                let layout_bounds = text_layout.bounds();
                let in_band = event.position.y >= layout_bounds.top()
                    && event.position.y <= layout_bounds.bottom();
                let exact = layout_bounds.contains(&event.position);
                if !exact && !in_band {
                    return;
                }
                if in_band
                    && !exact
                    && (event.position.x < layout_bounds.left()
                        || event.position.x > layout_bounds.right())
                {
                    return;
                }
                let offset = text_layout
                    .index_for_position(event.position)
                    .unwrap_or_else(|e| e);
                host.update(cx, |state, cx| {
                    state.mouse_move(block_ix, offset, cx);
                });
            });
        }

        if pending {
            let host = host.clone();
            window.on_mouse_event(move |_event: &MouseUpEvent, phase, _window, cx| {
                if phase == DispatchPhase::Capture {
                    host.update(cx, |state, cx| state.mouse_up(cx));
                }
            });
        }

        Element::paint(
            styled,
            None,
            inspector_id,
            bounds,
            &mut (),
            &mut (),
            window,
            cx,
        );
    }
}
