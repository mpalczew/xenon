use super::workspaces::WorkspacePickerMode;
use super::*;
use crate::workspace_create::{WorkspaceCreateEvent, WorkspaceCreateView};

impl XenonApp {
    pub(crate) fn on_workspace_menu_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(menu) = self.workspace_menu.as_ref() else {
            return false;
        };
        match event.keystroke.key.as_str() {
            "escape" => self.workspace_menu = None,
            "up" => {
                if let Some(menu) = self.workspace_menu.as_mut() {
                    menu.selected = 0;
                }
            }
            "down" => {
                if let Some(menu) = self.workspace_menu.as_mut() {
                    menu.selected = 1;
                }
            }
            "enter" => {
                let selected = menu.selected;
                self.workspace_menu = None;
                if selected == 0 {
                    self.open_workspace_creator(window, cx);
                } else {
                    self.open_workspace_picker(WorkspacePickerMode::Plus, window, cx);
                }
            }
            _ => return false,
        };
        cx.notify();
        true
    }

    pub(crate) fn open_workspace_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_palettes();
        self.deferred.restore_pane = self.focused_pane(window, cx);
        let creator = cx.new(WorkspaceCreateView::new);
        self._workspace_create_sub = Some(cx.subscribe(&creator, Self::on_workspace_create_event));
        self.workspace_create = Some(creator);
        cx.notify();
    }

    pub(crate) fn open_workspace_picker_from_plus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workspace_menu = None;
        self.open_workspace_picker(WorkspacePickerMode::Plus, window, cx);
    }

    fn on_workspace_create_event(
        &mut self,
        _view: Entity<WorkspaceCreateView>,
        event: &WorkspaceCreateEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorkspaceCreateEvent::Create { name, parent } => {
                let root = parent.join(name);
                match std::fs::create_dir(&root) {
                    Ok(()) => {
                        self.workspace_create = None;
                        self.deferred.restore_pane = None;
                        self.register_workspace(root, cx);
                    }
                    Err(error) => log::error!("create workspace {}: {error}", root.display()),
                }
            }
            WorkspaceCreateEvent::Dismissed => {
                self.workspace_create = None;
                self.deferred.pending_focus = self.deferred.restore_pane.take();
                cx.notify();
            }
        }
    }
}
