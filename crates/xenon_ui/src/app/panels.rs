use super::*;
use crate::resize::{
    DragResult, ResizeEdge, WORKSPACES_HEADER_H, resolve_drag, resolve_section_height,
};
use gpui::DragMoveEvent;
use xenon_core::SplitAxis;

impl XenonApp {
    /// Persist Workspaces/Files section prefs into settings.json.
    pub(super) fn persist_section_prefs(&self) {
        if let Err(error) = xenon_store::update_settings(|settings| {
            settings.workspaces_collapsed = self.workspaces_collapsed;
            settings.files_open = self.file_browser.is_open();
            settings.workspaces_section_height = self.workspaces_section_height;
        }) {
            log::error!("persist section prefs failed: {error}");
        }
    }

    pub(crate) fn sidebar_visible(&self) -> bool {
        !self.sidebar_collapsed
    }

    /// Snapshot live tree + sidebar into session and save.
    pub(super) fn save_layout(&mut self, id: WorkspaceId) {
        let content = self
            .contents
            .get(&id)
            .map(|c| c.snapshot())
            .unwrap_or_default();
        let session = SessionState {
            sidebar_visible: !self.sidebar_collapsed,
            sidebar_width: xenon_core::clamp_sidebar(self.sidebar_width),
            content,
        };
        self.sessions.insert(id, session.clone());
        save_session(id, &session, "save_layout");
    }

    pub(super) fn apply_sidebar(&mut self, session: &SessionState) {
        self.sidebar_collapsed = !session.sidebar_visible;
        self.sidebar_width = xenon_core::clamp_sidebar(session.sidebar_width);
    }

    pub(super) fn toggle_sidebar_panel(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        cx.notify();
    }

    /// ⌘J / pending Terminal focus: focus a terminal in the focused leaf, or create one.
    /// Prefer the leaf's active tab when it is already a terminal (so Run Task on a
    /// newly opened tab does not jump back to the first terminal).
    pub(super) fn focus_or_new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_gpui_leaf(window, cx);
        self.browser_focused = false;
        self.deferred.last_font_pane = FontPane::Terminal;
        if let Some(content) = self.active_content()
            && let Some(leaf) = content.focused_leaf()
        {
            let idx = if leaf.active_tab().is_some_and(|t| t.is_terminal()) {
                Some(leaf.active)
            } else {
                leaf.tabs.iter().position(|t| t.is_terminal())
            };
            if let Some(idx) = idx {
                let pane = leaf.id;
                self.activate_tab_in_pane(pane, idx, window, cx);
                return;
            }
        }
        self.new_terminal(window, cx);
    }

    /// ⌘⇧E / ⌘2: focus last editor if any, and make that leaf the session target.
    pub(super) fn focus_or_reveal_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browser_focused = false;
        self.deferred.last_font_pane = FontPane::Editor;
        let located = self.active_editor().and_then(|editor| {
            let (ws, tab) = self.locate_editor(&editor)?;
            let (pane, idx) = self.contents.get(&ws)?.root.as_ref()?.find_tab(tab)?;
            Some((pane, idx))
        });
        if let Some((pane, idx)) = located {
            self.activate_tab_in_pane(pane, idx, window, cx);
            return;
        }
        if let Some(editor) = self.active_editor() {
            editor.read(cx).focus_handle(cx).focus(window, cx);
            cx.notify();
        }
    }

    pub(super) fn on_resize_drag(
        &mut self,
        event: &DragMoveEvent<ResizeEdge>,
        _origin_x: gpui::Pixels,
        edge: ResizeEdge,
        cx: &mut Context<Self>,
    ) {
        match edge {
            ResizeEdge::Sidebar => {
                let raw = f32::from(event.event.position.x - event.bounds.origin.x);
                let available = f32::from(event.bounds.size.width);
                match resolve_drag(ResizeEdge::Sidebar, raw, available) {
                    DragResult::Width(width) => {
                        if self.sidebar_collapsed {
                            self.sidebar_collapsed = false;
                        }
                        if set_if_changed(&mut self.sidebar_width, width) {
                            self.layout_dirty = true;
                            cx.notify();
                        }
                    }
                    DragResult::ClosePrimary => {
                        if !self.sidebar_collapsed {
                            self.sidebar_collapsed = true;
                            self.layout_dirty = true;
                            cx.notify();
                        }
                    }
                }
            }
            ResizeEdge::SidebarSections => {
                let raw =
                    f32::from(event.event.position.y - event.bounds.origin.y) - WORKSPACES_HEADER_H;
                let available = f32::from(event.bounds.size.height);
                if available < 1.0 {
                    return;
                }
                let height = resolve_section_height(raw, available);
                let changed = match self.workspaces_section_height {
                    Some(h) => (h - height).abs() >= 0.5,
                    None => true,
                };
                if changed {
                    self.workspaces_section_height = Some(height);
                    self.layout_dirty = true;
                    cx.notify();
                }
            }
            ResizeEdge::Content { axis, first_leaf } => {
                let pos = match axis {
                    SplitAxis::Horizontal => {
                        f32::from(event.event.position.x - event.bounds.origin.x)
                    }
                    SplitAxis::Vertical => {
                        f32::from(event.event.position.y - event.bounds.origin.y)
                    }
                };
                let avail = match axis {
                    SplitAxis::Horizontal => f32::from(event.bounds.size.width),
                    SplitAxis::Vertical => f32::from(event.bounds.size.height),
                };
                if avail < 1.0 {
                    return;
                }
                let ratio = (pos / avail).clamp(0.15, 0.85);
                if let Some(id) = self.active
                    && let Some(content) = self.contents.get_mut(&id)
                    && let Some(root) = content.root.as_mut()
                    && root.set_ratio_for_split(first_leaf, ratio)
                {
                    self.layout_dirty = true;
                    cx.notify();
                }
            }
        }
    }

    pub(super) fn finish_resize(&mut self, _cx: &mut Context<Self>) {
        if !self.layout_dirty {
            return;
        }
        self.layout_dirty = false;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        self.persist_section_prefs();
    }

    pub(crate) fn sidebar_width_px(&self) -> f32 {
        xenon_core::clamp_sidebar(self.sidebar_width)
    }
}

fn set_if_changed(slot: &mut f32, value: f32) -> bool {
    if (*slot - value).abs() < 0.5 {
        return false;
    }
    *slot = value;
    true
}
