//! Command palette, stream jump, and keyboard-help wiring.

use super::*;
use crate::command_palette::{CommandPaletteEvent, CommandPaletteView, PaletteMode};
use xero_core::WorkspaceId;

impl XeroApp {
    pub(super) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette_mode(PaletteMode::Commands, window, cx);
    }

    pub(super) fn open_stream_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette_mode(PaletteMode::Streams, window, cx);
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
        self.finder = None;
        self.task_picker = None;
        self.workspace_picker = None;
        self.browser_focused = false;
        self.restore_pane = self.focused_pane(window, cx);
        let streams = self.stream_palette_rows();
        let workspaces = self.workspace_palette_rows();
        let picker = cx.new(|cx| CommandPaletteView::new(mode, streams, workspaces, cx));
        self._command_palette_sub = Some(cx.subscribe(&picker, Self::on_command_palette_event));
        self.command_palette = Some(picker);
        cx.notify();
    }

    fn stream_palette_rows(&self) -> Vec<(StreamId, String)> {
        let mut rows = Vec::new();
        for ws in &self.registry.workspaces {
            for &stream in &ws.streams {
                let name = self.stream_name(stream);
                rows.push((stream, format!("{} / {name}", ws.name)));
            }
        }
        rows
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
                // Defer run to next frame so we have a Window via pending.
                self.pending_command = Some(*id);
                cx.notify();
            }
            CommandPaletteEvent::ActivateStream(id) => {
                self.command_palette = None;
                self.pending_stream = Some(*id);
                cx.notify();
            }
            CommandPaletteEvent::ActivateWorkspace(id) => {
                self.command_palette = None;
                if let Some(stream) = self
                    .registry
                    .workspace(*id)
                    .and_then(|w| w.streams.first().copied())
                {
                    self.pending_stream = Some(stream);
                }
                cx.notify();
            }
            CommandPaletteEvent::Dismissed => {
                self.command_palette = None;
                self.pending_focus = self.restore_pane.take();
                cx.notify();
            }
        }
    }
}
