//! Files-tree context menu (right-click / keyboard).

use super::*;

impl XenonApp {
    pub(crate) fn open_browser_menu(
        &mut self,
        path: Option<PathBuf>,
        is_dir: bool,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.active_workspace().is_none() {
            return;
        }
        self.tab_menu = None;
        self.browser_menu = Some(crate::app::BrowserContextMenu {
            path,
            is_dir,
            position,
            selected: 0,
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(crate) fn dismiss_browser_menu(&mut self, cx: &mut Context<Self>) {
        if self.browser_menu.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn on_browser_menu_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(menu) = self.browser_menu.as_ref() else {
            return false;
        };
        let items = browser_menu_actions(menu);
        if items.is_empty() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                self.dismiss_browser_menu(cx);
                true
            }
            "up" => {
                if let Some(m) = self.browser_menu.as_mut() {
                    m.selected = m.selected.saturating_sub(1);
                    cx.notify();
                }
                true
            }
            "down" => {
                if let Some(m) = self.browser_menu.as_mut() {
                    m.selected = (m.selected + 1).min(items.len().saturating_sub(1));
                    cx.notify();
                }
                true
            }
            "enter" => {
                let selected = menu.selected.min(items.len() - 1);
                let action = items[selected];
                let path = menu.path.clone();
                let is_dir = menu.is_dir;
                self.dismiss_browser_menu(cx);
                if action == BrowserMenuAction::Delete {
                    if let Some(p) = path {
                        self.confirm_delete_path(p, is_dir, window, cx);
                    }
                } else {
                    self.run_browser_menu_action(action, path, is_dir, cx);
                }
                true
            }
            _ => false,
        }
    }

    fn run_browser_menu_action(
        &mut self,
        action: BrowserMenuAction,
        path: Option<PathBuf>,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let root = self.active.and_then(|id| self.workspace_root(id));
        let target = path.clone().or_else(|| root.clone());
        let parent_for_create = match (&path, is_dir) {
            (Some(p), true) => p.clone(),
            (Some(p), false) => p
                .parent()
                .map(|p| p.to_path_buf())
                .or_else(|| root.clone())
                .unwrap_or_else(|| PathBuf::from(".")),
            (None, _) => root.clone().unwrap_or_else(|| PathBuf::from(".")),
        };
        match action {
            BrowserMenuAction::NewFile => self.new_file_in_dir(parent_for_create, cx),
            BrowserMenuAction::NewFolder => self.new_folder_in_dir(parent_for_create, cx),
            BrowserMenuAction::CopyPath => {
                if let Some(p) = target {
                    Self::copy_path_abs(&p, cx);
                }
            }
            BrowserMenuAction::CopyRelativePath => {
                if let Some(p) = target {
                    self.copy_path_relative(&p, cx);
                }
            }
            BrowserMenuAction::CopyName => {
                if let Some(p) = &path {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.to_string_lossy().into_owned());
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(name));
                }
            }
            BrowserMenuAction::RevealInFinder => {
                if let Some(p) = target {
                    Self::reveal_in_finder(&p);
                }
            }
            BrowserMenuAction::OpenInDefaultApp => {
                if let Some(p) = path.filter(|_p| !is_dir) {
                    cx.open_with_system(&p);
                }
            }
            BrowserMenuAction::OpenInEditor => {
                if let Some(p) = path.filter(|_p| !is_dir) {
                    self.open_editor(p, true, cx);
                }
            }
            BrowserMenuAction::Delete => {}
        }
    }

    /// Confirm, move to Trash, close matching editor tabs, refresh the tree.
    fn confirm_delete_path(
        &mut self,
        path: PathBuf,
        is_dir: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let kind = if is_dir { "folder" } else { "file" };
        let message = format!("Move the {kind} \"{name}\" to the Trash?");
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some("You can restore it from the Trash later."),
            &["Move to Trash", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            let Ok(choice) = answer.await else {
                return;
            };
            if choice != 0 {
                return;
            }
            this.update(cx, |this, cx| this.delete_path(path, is_dir, cx))
                .ok();
        })
        .detach();
    }

    fn delete_path(&mut self, path: PathBuf, is_dir: bool, cx: &mut Context<Self>) {
        if let Err(error) = trash::delete(&path) {
            log::error!("move to trash failed for {}: {error}", path.display());
            return;
        }
        if let Some(id) = self.active {
            let tabs = self.editor_tabs_under(id, &path, is_dir);
            if !tabs.is_empty() {
                self.drop_tabs(id, &tabs, None, cx);
            }
        }
        if let Some(id) = self.active
            && let Some(root) = self.workspace_root(id)
        {
            self.reindex(root, true, cx);
        }
        cx.notify();
    }

    /// Open editor tabs for `path`, or any file under it when `is_dir`.
    fn editor_tabs_under(&self, workspace: WorkspaceId, path: &Path, is_dir: bool) -> Vec<TabId> {
        let mut out = Vec::new();
        let Some(root) = self.contents.get(&workspace).and_then(|c| c.root.as_ref()) else {
            return out;
        };
        collect_editor_tabs_under(root, path, is_dir, &mut out);
        out
    }

    pub(crate) fn render_browser_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let menu = self.browser_menu.as_ref()?;
        let colors = cx.theme().colors().clone();
        let position = menu.position;
        let selected = menu.selected;
        let path = menu.path.clone();
        let is_dir = menu.is_dir;
        let items = browser_menu_actions(menu);
        if items.is_empty() {
            return None;
        }

        let mut menu_box = div()
            .occlude()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_md()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation());

        for (i, action) in items.into_iter().enumerate() {
            let label = browser_menu_label(action);
            let is_sel = i == selected;
            let path = path.clone();
            menu_box = menu_box.child(crate::tabs::menu::menu_item(
                SharedString::from(format!("browser-menu-{i}")),
                label,
                &colors,
                is_sel,
                cx.listener(move |this, _, window, cx| {
                    this.dismiss_browser_menu(cx);
                    if action == BrowserMenuAction::Delete {
                        if let Some(p) = path.clone() {
                            this.confirm_delete_path(p, is_dir, window, cx);
                        }
                    } else {
                        this.run_browser_menu_action(action, path.clone(), is_dir, cx);
                    }
                }),
            ));
        }

        Some(
            div()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.dismiss_browser_menu(cx)),
                )
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(|this, _, _, cx| this.dismiss_browser_menu(cx)),
                )
                .child(
                    gpui::deferred(gpui::anchored().position(position).child(menu_box))
                        .with_priority(1),
                ),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BrowserMenuAction {
    NewFile,
    NewFolder,
    CopyPath,
    CopyRelativePath,
    CopyName,
    RevealInFinder,
    OpenInDefaultApp,
    OpenInEditor,
    Delete,
}

fn browser_menu_actions(menu: &crate::app::BrowserContextMenu) -> Vec<BrowserMenuAction> {
    let mut items = vec![
        BrowserMenuAction::NewFile,
        BrowserMenuAction::NewFolder,
        BrowserMenuAction::CopyPath,
        BrowserMenuAction::CopyRelativePath,
    ];
    if menu.path.is_some() {
        items.push(BrowserMenuAction::CopyName);
    }
    items.push(BrowserMenuAction::RevealInFinder);
    if menu.path.is_some() && !menu.is_dir {
        items.extend([
            BrowserMenuAction::OpenInEditor,
            BrowserMenuAction::OpenInDefaultApp,
        ]);
    }
    if menu.path.is_some() {
        items.push(BrowserMenuAction::Delete);
    }
    items
}

fn browser_menu_label(action: BrowserMenuAction) -> &'static str {
    match action {
        BrowserMenuAction::NewFile => "New File…",
        BrowserMenuAction::NewFolder => "New Folder…",
        BrowserMenuAction::CopyPath => "Copy Path",
        BrowserMenuAction::CopyRelativePath => "Copy Relative Path",
        BrowserMenuAction::CopyName => "Copy Name",
        BrowserMenuAction::RevealInFinder => "Reveal in Finder",
        BrowserMenuAction::OpenInDefaultApp => "Open in Default App",
        BrowserMenuAction::OpenInEditor => "Open in Editor",
        BrowserMenuAction::Delete => "Move to Trash",
    }
}

fn collect_editor_tabs_under(node: &LiveNode, path: &Path, is_dir: bool, out: &mut Vec<TabId>) {
    match node {
        LiveNode::Leaf(leaf) => {
            for tab in &leaf.tabs {
                if let Some(editor_path) = tab.editor_path()
                    && (editor_path == path || (is_dir && editor_path.starts_with(path)))
                {
                    out.push(tab.id());
                }
            }
        }
        LiveNode::Split { first, second, .. } => {
            collect_editor_tabs_under(first, path, is_dir, out);
            collect_editor_tabs_under(second, path, is_dir, out);
        }
    }
}
