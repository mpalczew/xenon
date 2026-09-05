//! Command palette and keyboard-help wiring.

use super::*;
use crate::command_palette::{CommandPaletteEvent, CommandPaletteView, PaletteMode};
use xenon_core::WorkspaceId;

impl XenonApp {
    pub(super) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette_mode(PaletteMode::Commands, window, cx);
    }

    pub(super) fn open_keyboard_help(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette_mode(PaletteMode::Help, window, cx);
    }

    fn open_palette_mode(
        &mut self,
        mode: PaletteMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_palettes();
        self.browser_focused = false;
        self.deferred.restore_pane = self.focused_pane(window, cx);
        let workspaces = self.workspace_palette_rows();
        let picker = cx.new(|cx| CommandPaletteView::new(mode, workspaces, cx));
        self._command_palette_sub = Some(cx.subscribe(&picker, Self::on_command_palette_event));
        self.command_palette = Some(picker);
        cx.notify();
    }

    fn workspace_palette_rows(&self) -> Vec<(WorkspaceId, String)> {
        self.registry
            .workspaces
            .iter()
            .map(|w| (w.id, w.name.clone()))
            .collect()
    }

    fn on_command_palette_event(
        &mut self,
        _picker: Entity<CommandPaletteView>,
        event: &CommandPaletteEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            CommandPaletteEvent::Run(id) => {
                self.command_palette = None;
                // Restore before the command runs (drain order: focus then command).
                self.deferred.pending_focus = self
                    .deferred
                    .restore_pane
                    .take()
                    .or_else(|| Some(self.fallback_content_pane()));
                self.deferred.pending_command = Some(*id);
                cx.notify();
            }
            CommandPaletteEvent::ActivateWorkspace(id) => {
                self.command_palette = None;
                self.deferred.restore_pane = None;
                self.deferred.pending_workspace = Some(*id);
                cx.notify();
            }
            CommandPaletteEvent::Dismissed => {
                self.command_palette = None;
                self.deferred.pending_focus = self
                    .deferred
                    .restore_pane
                    .take()
                    .or_else(|| Some(self.fallback_content_pane()));
                cx.notify();
            }
        }
    }
}
