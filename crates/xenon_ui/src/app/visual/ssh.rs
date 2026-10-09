use super::*;
use gpui::AppContext;
use xenon_core::{PaneId, TabId, WorkspaceRec};
use xenon_editor::EditorView;

pub(super) fn workspace(
    app: &mut XenonApp,
    unavailable: bool,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    let ssh = xenon_ssh::SshWorkspace::new("devbox".into(), "/home/alex/project".into()).unwrap();
    let record = WorkspaceRec::remote(ssh);
    let id = record.id;
    let root = record.root.clone();
    let path = root.join("src/main.rs");
    let view = cx.new(|cx| {
        EditorView::visual_remote(
            path.clone(),
            "fn main() {\n    println!(\"Remote workspace\");\n}\n",
            unavailable,
            cx,
        )
    });
    app.registry.workspaces.push(record);
    app.active = Some(id);
    app.sidebar_collapsed = false;
    app.contents.insert(
        id,
        super::super::LiveContent {
            root: Some(super::super::LiveNode::Leaf(super::super::LiveLeaf {
                id: PaneId(1),
                active: 0,
                parked: false,
                tabs: vec![super::super::LiveTab::Editor {
                    id: TabId(1),
                    path,
                    name: "main.rs".into(),
                    view,
                }],
            })),
            focused: Some(PaneId(1)),
        },
    );
    app.file_browser.install_remote(
        root.clone(),
        vec![
            xenon_ssh::RemoteEntry {
                path: "src".into(),
                is_dir: true,
                is_symlink: false,
            },
            xenon_ssh::RemoteEntry {
                path: "src/main.rs".into(),
                is_dir: false,
                is_symlink: false,
            },
            xenon_ssh::RemoteEntry {
                path: "Cargo.toml".into(),
                is_dir: false,
                is_symlink: false,
            },
        ],
    );
    app.file_browser.reveal_dir(&root, &root.join("src"));
    app.focus_workspace_leaf(Some(window), cx);
    cx.notify();
}
