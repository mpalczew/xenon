use super::*;
use crate::resize::{DragResult, ResizeEdge, resolve_drag};
use gpui::DragMoveEvent;

impl XeroApp {
    pub(crate) fn sidebar_visible(&self) -> bool {
        !self.sidebar_collapsed
    }

    pub(crate) fn terminal_visible(&self) -> bool {
        // Panel open even with zero tabs (empty state: press ⌘N).
        !self.terminal_collapsed
    }

    pub(crate) fn editor_visible(&self) -> bool {
        !self.editor_collapsed && self.active.is_some()
    }

    /// Active workspace / stream label for the toolbar breadcrumb.
    pub(crate) fn breadcrumb_label(&self) -> Option<String> {
        let stream = self.active?;
        let workspace = self.workspace_of(stream)?;
        Some(format!("{} / {}", workspace.name, self.stream_name(stream)))
    }

    /// Live visibility + widths as a `Layout` (template for new streams).
    pub(super) fn current_layout(&self) -> Layout {
        Layout {
            terminal_visible: !self.terminal_collapsed,
            editor_visible: !self.editor_collapsed,
            sidebar_visible: !self.sidebar_collapsed,
            sidebar_width: self.sidebar_width,
            tree_width: self.tree_width,
            terminal_width: self.terminal_width,
        }
        .clamp_widths()
    }

    /// Write live layout into the stream session and save.
    pub(super) fn save_layout(&mut self, id: StreamId) {
        let layout = self.current_layout();
        if let Some(stream) = self.streams.get_mut(&id) {
            stream.session.layout = layout;
        }
        if self.streams.contains_key(&id)
            && let Some(workspace) = self.workspace_of(id).map(|w| w.id)
        {
            save_session(workspace, &self.streams[&id], "save_layout");
        }
    }

    /// Apply a stored layout to the live chrome fields.
    pub(super) fn apply_layout(&mut self, layout: Layout) {
        let layout = layout.clamp_widths();
        self.terminal_collapsed = !layout.terminal_visible;
        self.editor_collapsed = !layout.editor_visible;
        self.sidebar_collapsed = !layout.sidebar_visible;
        self.sidebar_width = layout.sidebar_width;
        self.tree_width = layout.tree_width;
        self.terminal_width = layout.terminal_width;
    }

    pub(super) fn toggle_terminal_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_collapsed = !self.terminal_collapsed;
        if !self.terminal_collapsed {
            self.ensure_terminal(cx);
            if let Some(terminal) = self.active_terminal() {
                terminal.read(cx).focus_handle(cx).focus(window, cx);
            }
        }
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        cx.notify();
    }

    pub(super) fn toggle_editor_panel(&mut self, cx: &mut Context<Self>) {
        self.editor_collapsed = !self.editor_collapsed;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        cx.notify();
    }

    pub(super) fn toggle_sidebar_panel(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        cx.notify();
    }

    /// Ensure the active stream has a live terminal tab (no focus change).
    pub(super) fn ensure_terminal(&mut self, cx: &mut Context<Self>) {
        let id = self.active;
        if let Some(id) = id
            && !self.terminals.contains_key(&id)
            && let Some(root) = self.stream_root(id)
        {
            let terminal = self.spawn_terminal(root, id, cx);
            self.terminals.insert(
                id,
                TerminalStack {
                    tabs: vec![terminal],
                    active: 0,
                },
            );
        }
    }

    pub(super) fn on_resize_drag(
        &mut self,
        event: &DragMoveEvent<ResizeEdge>,
        origin_x: gpui::Pixels,
        edge: ResizeEdge,
        cx: &mut Context<Self>,
    ) {
        let raw = f32::from(event.event.position.x - origin_x);
        let available = f32::from(event.bounds.size.width);
        match resolve_drag(edge, raw, available) {
            DragResult::Width(width) => {
                // Dragging back past the snap threshold reopens a closed pane.
                let reopened = self.reopen_for_edge(edge, cx);
                let changed = match edge {
                    ResizeEdge::Sidebar => set_if_changed(&mut self.sidebar_width, width),
                    ResizeEdge::Tree => set_if_changed(&mut self.tree_width, width),
                    ResizeEdge::Terminal => set_if_changed(&mut self.terminal_width, width),
                };
                if changed || reopened {
                    self.layout_dirty = true;
                    cx.notify();
                }
            }
            DragResult::ClosePrimary => self.snap_close_primary(edge, cx),
            DragResult::CloseSecondary => self.snap_close_secondary(edge, cx),
        }
    }

    /// Reopen a snap-closed pane when the drag returns to a valid width.
    fn reopen_for_edge(&mut self, edge: ResizeEdge, cx: &mut Context<Self>) -> bool {
        match edge {
            ResizeEdge::Sidebar if self.sidebar_collapsed => {
                self.sidebar_collapsed = false;
                true
            }
            ResizeEdge::Tree if !self.file_browser.is_open() => {
                self.file_browser.open();
                true
            }
            ResizeEdge::Terminal => {
                let mut changed = false;
                if self.terminal_collapsed {
                    self.terminal_collapsed = false;
                    self.ensure_terminal(cx);
                    changed = true;
                }
                if self.editor_collapsed {
                    self.editor_collapsed = false;
                    changed = true;
                }
                changed
            }
            _ => false,
        }
    }

    /// Snap-close the pane owned by this resize edge; keep last good width.
    fn snap_close_primary(&mut self, edge: ResizeEdge, cx: &mut Context<Self>) {
        let closed = match edge {
            ResizeEdge::Sidebar if !self.sidebar_collapsed => {
                self.sidebar_collapsed = true;
                true
            }
            ResizeEdge::Tree if self.file_browser.is_open() => {
                self.file_browser.close();
                true
            }
            ResizeEdge::Terminal if !self.terminal_collapsed => {
                self.terminal_collapsed = true;
                true
            }
            _ => false,
        };
        if closed {
            self.layout_dirty = true;
            cx.notify();
        }
    }

    /// Snap-close the pane to the right of the drag edge (editor).
    fn snap_close_secondary(&mut self, edge: ResizeEdge, cx: &mut Context<Self>) {
        if !matches!(edge, ResizeEdge::Terminal) || self.editor_collapsed {
            return;
        }
        self.editor_collapsed = true;
        self.layout_dirty = true;
        cx.notify();
    }

    pub(super) fn finish_resize(&mut self, _cx: &mut Context<Self>) {
        if !self.layout_dirty {
            return;
        }
        self.layout_dirty = false;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
    }

    pub(crate) fn sidebar_width_px(&self) -> f32 {
        xero_core::clamp_sidebar(self.sidebar_width)
    }

    pub(crate) fn tree_width_px(&self) -> f32 {
        xero_core::clamp_tree(self.tree_width)
    }

    pub(crate) fn terminal_width_px(&self) -> f32 {
        xero_core::clamp_terminal(self.terminal_width)
    }
}

fn set_if_changed(slot: &mut f32, value: f32) -> bool {
    if (*slot - value).abs() < 0.5 {
        return false;
    }
    *slot = value;
    true
}
