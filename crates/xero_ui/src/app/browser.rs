use super::*;

impl XeroApp {
    /// Whether the file-tree sidebar is open (a collapsible sidebar beside the
    /// editor, toggled by the toolbar/tab-bar buttons and cmd-e).
    pub(crate) fn is_browsing(&self) -> bool {
        self.active.is_some() && self.file_browser.is_open()
    }

    pub(crate) fn toggle_browser(&mut self, cx: &mut Context<Self>) {
        if self.file_browser.is_open() {
            self.file_browser.close();
            cx.notify();
            return;
        }
        self.show_browser(cx);
    }

    fn show_browser(&mut self, cx: &mut Context<Self>) {
        if !self.reveal_active_file(cx) {
            self.file_browser.open();
            self.editor_collapsed = false;
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
        let Some(root) = self.active.and_then(|id| self.stream_root(id)) else {
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

    /// The file-tree sidebar: a header naming the workspace, then a lazy,
    /// expandable tree rooted at the active stream's working dir.
    pub(super) fn render_tree_sidebar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = cx.theme().colors().clone();
        let Some(id) = self.active else {
            return div().into_any_element();
        };
        let Some(root) = self.stream_root(id) else {
            return div().into_any_element();
        };
        let workspace = self
            .workspace_of(id)
            .map(|w| w.name.clone())
            .unwrap_or_default();
        let open_file = self
            .active_editor()
            .map(|view| view.read(cx).path().to_path_buf());
        let rows = self.file_browser.rows(&root, open_file.as_deref());

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .truncate()
                    .child(workspace.to_uppercase()),
            )
            .child(
                div()
                    .id("tree-collapse")
                    .w(px(22.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .text_color(colors.text_muted)
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.element_hover).text_color(colors.text))
                    .child("x")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_browser(cx))),
            );

        div()
            .w(px(240.))
            .flex_none()
            .flex()
            .flex_col()
            .min_h_0()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(header)
            .child(
                div()
                    .id("browser")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_2()
                    .children(rows.into_iter().map(|row| self.tree_row(row, cx))),
            )
            .into_any_element()
    }

    fn tree_row(&self, row: TreeRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = cx.theme().colors().clone();
        let indent = px(8. + row.depth as f32 * 16.);
        let marker = dir_marker(row.is_dir, row.expanded);
        let icon = file_icon(&row);
        let background = if row.is_open {
            colors.element_selected
        } else {
            gpui::transparent_black()
        };
        let id = SharedString::from(row.path.to_string_lossy().into_owned());
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .pl(indent)
            .pr_2()
            .py(px(2.))
            .text_sm()
            .text_color(if row.is_open {
                colors.text
            } else {
                colors.text_muted
            })
            .bg(background)
            .cursor_pointer()
            .hover(|s| s.bg(colors.element_hover))
            .child(div().w(px(12.)).text_color(colors.text_muted).child(marker))
            .child(div().w(px(16.)).text_color(colors.text_muted).child(icon))
            .child(div().truncate().child(row.name.clone()))
            .on_click(cx.listener(move |this, _, _window, cx| {
                if row.is_dir {
                    this.toggle_dir(row.path.clone(), cx);
                } else {
                    this.open_editor(row.path.clone(), true, cx);
                }
            }))
            .into_any_element()
    }

    pub(super) fn open_palette(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.active.and_then(|id| self.stream_root(id)) else {
            return;
        };
        let finder = cx.new(|cx| FinderView::new(root, cx));
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
                if let Some(root) = self.active.and_then(|id| self.stream_root(id)) {
                    self.open_editor(root.join(relative), true, cx);
                }
            }
            FinderEvent::RevealDir(relative) => {
                if let Some(root) = self.active.and_then(|id| self.stream_root(id)) {
                    self.reveal_dir(&root, &root.join(relative), cx);
                }
            }
            FinderEvent::Dismissed => {
                self.finder = None;
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
        self.file_browser.reveal_dir(root, dir);
        self.editor_collapsed = false;
        if let Some(id) = self.active {
            self.save_layout(id);
        }
        self.finder = None;
        cx.notify();
    }
}
