//! Apply Language Server settings from the Settings window without a restart.

use std::path::Path;

use gpui::{App, Context};
use xenon_store::LspSettings;

use super::super::XenonApp;
use super::super::remote::with_main;
use super::collect_editors;

/// Persist an LSP settings change, then restart servers under the new config.
pub(crate) fn settings_set_lsp(update: impl FnOnce(&mut LspSettings), cx: &mut App) {
    let mut saved = None;
    if let Err(error) = xenon_store::update_settings(|settings| {
        update(&mut settings.lsp);
        saved = Some(settings.lsp.clone());
    }) {
        log::warn!("save language server settings: {error}");
        return;
    }
    let Some(lsp) = saved else {
        return;
    };
    with_main(cx, |app, cx| app.reload_lsp(lsp, cx));
}

impl XenonApp {
    /// Stop every server and reattach open editors under `settings`.
    fn reload_lsp(&mut self, settings: LspSettings, cx: &mut Context<Self>) {
        let roots: Vec<_> = self
            .registry
            .workspaces
            .iter()
            .map(|workspace| workspace.root.clone())
            .collect();
        for root in &roots {
            self.lsp.stop_workspace(root);
        }
        self.lsp.documents.clear();
        self.lsp.settings = settings;
        let workspaces: Vec<_> = self.contents.keys().copied().collect();
        for workspace in workspaces {
            let mut editors = Vec::new();
            if let Some(node) = self.contents.get(&workspace).and_then(|c| c.root.as_ref()) {
                collect_editors(node, Path::new("/"), &mut editors);
            }
            for view in editors {
                view.update(cx, |editor, cx| {
                    editor.set_lsp_status(None, cx);
                    editor.set_lsp_decorations(Vec::new(), Vec::new(), cx);
                });
                self.lsp_attach_editor(workspace, &view, cx);
            }
        }
    }
}
