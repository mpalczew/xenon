use std::path::Path;

use gpui::Entity;
use xenon_editor::EditorView;
use xenon_lsp::LanguageFamily;

use super::super::{LiveNode, LiveTab};

pub(super) fn language_for_path(path: &Path) -> Option<(LanguageFamily, &'static str)> {
    match path.extension()?.to_str()? {
        "rs" => Some((LanguageFamily::Rust, "rust")),
        "ts" => Some((LanguageFamily::TypeScript, "typescript")),
        "tsx" => Some((LanguageFamily::TypeScript, "typescriptreact")),
        "js" | "mjs" | "cjs" => Some((LanguageFamily::TypeScript, "javascript")),
        "jsx" => Some((LanguageFamily::TypeScript, "javascriptreact")),
        _ => None,
    }
}

pub(super) fn has_root_marker(path: &Path, workspace: &Path, family: LanguageFamily) -> bool {
    let markers: &[&str] = match family {
        LanguageFamily::Rust => &["Cargo.toml"],
        LanguageFamily::TypeScript => &["package.json", "tsconfig.json", "jsconfig.json"],
    };
    let mut directory = path.parent();
    while let Some(candidate) = directory {
        if markers
            .iter()
            .any(|marker| candidate.join(marker).is_file())
        {
            return true;
        }
        if candidate == workspace {
            break;
        }
        directory = candidate.parent();
    }
    false
}

pub(super) fn executable_on_path(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| directory.join(command).is_file())
    })
}

pub(super) fn collect_editors(node: &LiveNode, root: &Path, out: &mut Vec<Entity<EditorView>>) {
    match node {
        LiveNode::Leaf(leaf) => {
            for tab in &leaf.tabs {
                if let LiveTab::Editor { path, view, .. } = tab
                    && path.starts_with(root)
                {
                    out.push(view.clone());
                }
            }
        }
        LiveNode::Split { first, second, .. } => {
            collect_editors(first, root, out);
            collect_editors(second, root, out);
        }
    }
}
