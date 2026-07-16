use super::*;

impl XeroApp {
    /// Whether the file tree is shown in the workspace panel (⌘E).
    pub(crate) fn is_browsing(&self) -> bool {
        self.active.is_some() && self.file_browser.is_open()
    }

    pub(crate) fn toggle_browser(&mut self, cx: &mut Context<Self>) {
        if self.file_browser.is_open() {
            self.file_browser.close();
            self.browser_focused = false;
            persist_section_prefs(self.workspaces_collapsed, false);
            cx.notify();
            return;
        }
        self.show_browser(cx);
    }

    pub(super) fn show_browser(&mut self, cx: &mut Context<Self>) {
        // Tree lives in the left workspace panel — ensure the panel is open.
        self.sidebar_collapsed = false;
        if !self.reveal_active_file(cx) {
            self.file_browser.open();
            persist_section_prefs(self.workspaces_collapsed, true);
            if let Some(id) = self.active {
                self.save_layout(id);
            }
            cx.notify();
        }
    }

    pub(super) fn reveal_active_file(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(view) = self.active_editor() else {
            return false;
        };
        let path = view.read(cx).path().to_path_buf();
        let Some(root) = self.active.and_then(|id| self.workspace_root(id)) else {
            return false;
        };
        if let Some(parent) = path.parent() {
            self.reveal_dir(&root, parent, cx);
            return true;
        }
        false
    }

    fn toggle_dir(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.file_browser.toggle_dir(path);
        cx.notify();
    }

    /// Files section in the workspace panel (header always; tree when expanded).
    pub(crate) fn render_files_section(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.is_browsing() {
            return self.files_section_header(true, cx).into_any_element();
        }
        let Some(id) = self.active else {
            return div().into_any_element();
        };
        let Some(root) = self.workspace_root(id) else {
            return div().into_any_element();
        };
        let open_file = self
            .active_editor()
            .map(|view| view.read(cx).path().to_path_buf());
        let rows = self.file_browser.rows(&root, open_file.as_deref());

        div()
            .id("workspace-tree")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .min_w_0()
            .child(self.files_section_header(false, cx))
            .child(
                div()
                    .id("browser")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .map(|(i, row)| self.tree_row(i, row, cx)),
                    ),
            )
            .into_any_element()
    }

    fn files_section_header(
        &self,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = cx.theme().colors().clone();
        let chevron = if collapsed {
            lucide_icons::Icon::ChevronRight
        } else {
            lucide_icons::Icon::ChevronDown
        };
        let border = if self.browser_focused && !collapsed {
            colors.border_focused
        } else {
            colors.border
        };
        div()
            .id("files-section-header")
            .flex()
            .items_center()
            .gap_1()
            .h(px(28.))
            .px_2()
            .border_t_1()
            .border_color(border)
            .cursor_pointer()
            .hover(|s| s.text_color(colors.text))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_browser(cx)))
            .child(
                div()
                    .w(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(colors.text_muted)
                    .child(crate::icons::icon(chevron, px(12.))),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.text_muted)
                    .child("Files"),
            )
    }

    fn tree_row(&self, index: usize, row: TreeRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = cx.theme().colors().clone();
        let indent = px(8. + row.depth as f32 * 16.);
        let marker = dir_marker(row.is_dir, row.expanded);
        let glyph = file_icon(&row);
        let kb = self.browser_focused && self.file_browser.cursor() == Some(index);
        let paint = crate::chrome::list_selection(&colors, row.is_open || kb);
        let id = SharedString::from(row.path.to_string_lossy().into_owned());
        const ICON: f32 = 12.;
        let chevron = div()
            .w(px(14.))
            .flex()
            .items_center()
            .justify_center()
            .text_color(colors.text_muted)
            .children(marker.map(|m| crate::icons::icon(m, px(ICON))));
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .pl(indent)
            .pr_2()
            .py(px(2.))
            .min_w_0()
            .text_sm()
            .font_weight(if row.is_open || kb {
                gpui::FontWeight::MEDIUM
            } else {
                gpui::FontWeight::NORMAL
            })
            .text_color(paint.foreground)
            .bg(paint.background)
            .border_l_2()
            .border_color(paint.accent)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
            .child(chevron)
            .child(
                div()
                    .w(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(colors.text_muted)
                    .child(crate::icons::icon(glyph, px(ICON))),
            )
            .child(div().flex_1().min_w_0().truncate().child(row.name.clone()))
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.browser_focused = false;
                this.file_browser.ensure_cursor();
                if row.is_dir {
                    this.toggle_dir(row.path.clone(), cx);
                } else {
                    this.open_editor(row.path.clone(), true, cx);
                }
            }))
            .into_any_element()
    }

    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.task_picker = None;
        self.workspace_picker = None;
        self.command_palette = None;
        self.open_palette_with_query(String::new(), window, cx);
    }

    /// Open cmd-p prefilled with `query` (used when a cmd-clicked name is
    /// ambiguous). Serves the cached index instantly and refreshes it in the
    /// background so newly created files show up next time.
    pub(super) fn open_palette_with_query(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.active.and_then(|id| self.workspace_root(id)) else {
            return;
        };
        self.task_picker = None;
        self.workspace_picker = None;
        self.command_palette = None;
        self.restore_pane = self.focused_pane(window, cx);
        self.reindex(root.clone(), true, cx);
        let index = self.file_indexes.get(&root).cloned();
        let finder = cx.new(|cx| FinderView::new(index, query, cx));
        self._finder_sub = Some(cx.subscribe(&finder, Self::on_finder_event));
        self.finder = Some(finder);
        cx.notify();
    }

    fn on_finder_event(
        &mut self,
        _finder: Entity<FinderView>,
        event: &FinderEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            FinderEvent::Selected(relative) => {
                self.restore_pane = None; // open_editor focuses the editor itself
                if let Some(root) = self.active.and_then(|id| self.workspace_root(id)) {
                    self.open_editor(root.join(relative), true, cx);
                }
            }
            FinderEvent::RevealDir(relative) => {
                self.restore_pane = None;
                if let Some(root) = self.active.and_then(|id| self.workspace_root(id)) {
                    self.reveal_dir(&root, &root.join(relative), cx);
                }
            }
            FinderEvent::Dismissed => {
                self.finder = None;
                // Re-focus the pre-finder pane on the next render (which has a Window).
                self.pending_focus = self.restore_pane.take();
                cx.notify();
            }
        }
    }

    /// Expand `dir` and every ancestor up to (but excluding) `root`, then show
    /// the browser so the path is visible.
    fn reveal_dir(
        &mut self,
        root: &std::path::Path,
        dir: &std::path::Path,
        cx: &mut Context<Self>,
    ) {
        self.sidebar_collapsed = false;
        self.file_browser.reveal_dir(root, dir);
        persist_section_prefs(self.workspaces_collapsed, self.file_browser.is_open());
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        self.finder = None;
        cx.notify();
    }
}
