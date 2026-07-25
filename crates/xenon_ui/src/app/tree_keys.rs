//! File-tree keyboard while the browser pane is focused.

use super::content_ops::FindSurface;
use super::*;
use crate::commands::CommandId;

impl XenonApp {
    /// File-tree keyboard while `browser_focused`.
    pub(super) fn on_browser_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.browser_focused {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                self.browser_focused = false;
                self.focus_editor(window, cx);
                true
            }
            "up" => {
                self.file_browser.move_cursor(-1, self.tree_row_count());
                cx.notify();
                true
            }
            "down" => {
                self.file_browser.move_cursor(1, self.tree_row_count());
                cx.notify();
                true
            }
            "left" => {
                self.tree_collapse_or_parent(cx);
                true
            }
            "right" => {
                self.tree_expand_or_enter(window, cx);
                true
            }
            "enter" => {
                self.tree_activate_cursor(window, cx);
                true
            }
            _ => false,
        }
    }

    pub(super) fn tree_rows(&self) -> Vec<crate::file_browser::TreeRow> {
        let Some(id) = self.active else {
            return Vec::new();
        };
        let Some(root) = self.workspace_root(id) else {
            return Vec::new();
        };
        self.file_browser.rows(&root, None)
    }

    fn tree_row_count(&self) -> usize {
        self.tree_rows().len()
    }

    fn tree_cursor_path(&self) -> Option<std::path::PathBuf> {
        let rows = self.tree_rows();
        let i = self.file_browser.cursor()?;
        rows.get(i).map(|r| r.path.clone())
    }

    fn tree_collapse_or_parent(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.tree_cursor_path() else {
            return;
        };
        if path.is_dir() && self.file_browser.is_expanded(&path) {
            self.file_browser.toggle_dir(path);
            cx.notify();
            return;
        }
        if let Some(parent) = path.parent() {
            let rows = self.tree_rows();
            self.file_browser.select_path_in(&rows, parent);
            cx.notify();
        }
    }

    fn tree_expand_or_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.tree_cursor_path() else {
            return;
        };
        if path.is_dir() {
            if !self.file_browser.is_expanded(&path) {
                self.file_browser.toggle_dir(path);
                cx.notify();
            }
        } else {
            self.tree_activate_cursor(window, cx);
        }
    }

    fn tree_activate_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.tree_cursor_path() else {
            return;
        };
        if path.is_dir() {
            self.file_browser.toggle_dir(path);
            cx.notify();
        } else {
            self.browser_focused = false;
            self.open_editor(path, true, cx);
            if let Some(editor) = self.active_editor() {
                editor.read(cx).focus_handle(cx).focus(window, cx);
            }
        }
    }

    /// Dispatch a command palette selection.
    pub(super) fn run_command(
        &mut self,
        id: CommandId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match id {
            CommandId::NewTerminal => self.new_terminal(window, cx),
            CommandId::OpenWorkspace => self.add_workspace(window, cx),
            CommandId::OpenFile => self.open_file_dialog(cx),
            CommandId::NewFile => self.new_file_dialog(cx),
            CommandId::GoToFile => self.open_palette(window, cx),
            CommandId::RunTask => self.open_task_picker(window, cx),
            CommandId::Save => self.save_active_editor(cx),
            CommandId::SaveAs => self.save_as_dialog(cx),
            CommandId::FindInFile => match self.active_find_surface() {
                FindSurface::Terminal(view) => view.update(cx, |t, cx| t.open_find(window, cx)),
                FindSurface::Editor(view) => view.update(cx, |e, cx| e.open_find(window, cx)),
                FindSurface::None => {}
            },
            CommandId::FindNext => match self.active_find_surface() {
                FindSurface::Terminal(view) => view.update(cx, |t, cx| t.find_next(cx)),
                FindSurface::Editor(view) => view.update(cx, |e, cx| e.find_next(cx)),
                FindSurface::None => {}
            },
            CommandId::FindPrevious => match self.active_find_surface() {
                FindSurface::Terminal(view) => view.update(cx, |t, cx| t.find_previous(cx)),
                FindSurface::Editor(view) => view.update(cx, |e, cx| e.find_previous(cx)),
                FindSurface::None => {}
            },
            CommandId::CloseFocusedTab => self.close_focused_tab(window, cx),
            CommandId::CloseWorkspace => self.close_active_workspace(window, cx),
            CommandId::FocusTerminal => self.focus_terminal(window, cx),
            CommandId::FocusEditor => self.focus_editor(window, cx),
            CommandId::FocusBrowser => self.focus_browser(window, cx),
            CommandId::FocusNextPane => self.focus_next_pane(window, cx),
            CommandId::NextWorkspace => self.next_workspace(window, cx),
            CommandId::PrevWorkspace => self.prev_workspace(window, cx),
            CommandId::NextTab => self.next_tab(window, cx),
            CommandId::PrevTab => self.prev_tab(window, cx),
            CommandId::GoBack => self.go_back(window, cx),
            CommandId::GoForward => self.go_forward(window, cx),
            CommandId::ToggleSidebar => self.toggle_sidebar_panel(cx),
            CommandId::ToggleBrowser => self.toggle_browser(cx),
            CommandId::ToggleTerminal => self.focus_or_new_terminal(window, cx),
            CommandId::ToggleEditor => self.focus_or_reveal_editor(window, cx),
            CommandId::SplitRight => self.split_right(window, cx),
            CommandId::SplitDown => self.split_down(window, cx),
            CommandId::ToggleSettings => self.toggle_settings_window(cx),
            CommandId::CommandPalette => self.open_command_palette(window, cx),
            CommandId::KeyboardHelp => self.open_keyboard_help(window, cx),
            CommandId::ZoomIn => self.nudge_font_size(1.0, window, cx),
            CommandId::ZoomOut => self.nudge_font_size(-1.0, window, cx),
            CommandId::ZoomReset => self.reset_font_size(window, cx),
        }
    }
}
