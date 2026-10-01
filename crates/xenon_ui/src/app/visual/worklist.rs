//! Worklist scenes: the list with sections, inline editors, and capture.

use gpui::{Context, Window};
use xenon_editor::worklist_file::Target;

use super::{Scene, XenonApp, chrome, workspace_root};

pub(super) fn capture(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    chrome::populated(app, scene, window, cx);
    if scene == Scene::WorklistCaptureSection {
        write(app, SECTIONED_WORKLIST);
        if let Some(workspace) = app.active {
            let next = Target::Section {
                index: 1,
                title: "Next".into(),
            };
            app.worklist_last_section.insert(workspace, next);
        }
    }
    app.capture_worklist(window, cx);
    let text = match scene {
        Scene::WorklistCaptureFilled => {
            Some("Fix focus after closing a split\nHappens in the right pane.")
        }
        Scene::WorklistCaptureLong => Some(
            "Fix focus after closing a split when the right pane owns the active terminal and a prompt is open\nHappens in the right pane.",
        ),
        _ => None,
    };
    if let Some(text) = text
        && let Some(workspace) = app.active
        && let Some(capture) = app.worklist_captures.get(&workspace)
    {
        capture.update(cx, |view, cx| view.visual_set_text(text, cx));
    }
}

pub(super) fn tab(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    chrome::populated(app, scene, window, cx);
    if scene == Scene::WorklistEmptyVirtual {
        if let Some(root) = workspace_root(app) {
            let _ = std::fs::remove_file(root.join(".xenon/worklist.md"));
        }
    } else {
        write(app, fixture(scene));
    }
    app.open_worklist(cx);
    if let Some(editor) = app.active_editor() {
        editor.update(cx, |editor, cx| match scene {
            Scene::WorklistSections => editor.visual_worklist_header(1, false, cx),
            Scene::WorklistSectionsFolded => editor.visual_worklist_header(2, true, cx),
            Scene::WorklistSectionRename => editor.visual_worklist_rename(1, cx),
            Scene::WorklistItemEdit => editor.visual_worklist_edit(cx),
            Scene::WorklistMarkdown => editor.visual_worklist_markdown(cx),
            Scene::WorklistInlineCapture | Scene::WorklistInlineLight => {
                editor.visual_worklist_capture(window, cx)
            }
            _ => {}
        });
    }
}

const SECTIONED_WORKLIST: &str = "# Worklist\n\n- Reply to the Zed maintainers about the GPUI pin\n\n## Now\n\n- [ ] **Fix focus after closing a split** (*regression*)\n  - Happens when the right pane owns the active terminal; see `focus_after_close`.\n\n- [ ] Phone remote: paste images from clipboard\n  - Reuse the attach path from the image work.\n\n## Next\n\n- Workspace search idea\n  - Show recently used workspaces first.\n\n- [ ] Theme side panel keyboard path\n\n- [x] Verify One Light contrast on diffs\n\n## Someday\n";

fn fixture(scene: Scene) -> &'static str {
    match scene {
        Scene::WorklistEmpty | Scene::WorklistEmptyLight => "# Worklist\n",
        Scene::WorklistSections
        | Scene::WorklistSectionsLight
        | Scene::WorklistSectionsFolded
        | Scene::WorklistSectionRename => SECTIONED_WORKLIST,
        _ => {
            "# Worklist\n\n- [ ] Fix focus after closing a split\n  - Happens when the right pane owns the active terminal.\n\n- Workspace search idea\n  - Show recently used workspaces first.\n"
        }
    }
}

fn write(app: &XenonApp, text: &str) {
    let Some(root) = workspace_root(app) else {
        return;
    };
    let path = root.join(".xenon/worklist.md");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, text);
}
