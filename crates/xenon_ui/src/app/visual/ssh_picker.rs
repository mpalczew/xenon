//! Open Workspace picker with `ssh://` text, with fixed offline host answers.

use gpui::{Context, Window};

use super::{Scene, XenonApp, overlays};
use crate::workspace_picker::VisualRemote as Remote;

const HOSTS: [&str; 3] = ["devbox", "pixelbook", "thinkpad"];
const FOLDER_QUERY: &str = "ssh://thinkpad/src/x";

pub(super) fn scene(
    app: &mut XenonApp,
    scene: Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    overlays::workspace_picker(app, window, cx);
    let Some(picker) = &app.workspace_picker else {
        return;
    };
    picker.update(cx, |picker, cx| {
        picker.visual_hosts(&HOSTS, cx);
        match scene {
            Scene::SshHostPicker => picker.visual_query("think", cx),
            Scene::SshHostComplete => picker.visual_query("ssh://thin", cx),
            Scene::SshHostResults => picker.visual_remote(FOLDER_QUERY, listing(), cx),
            Scene::SshHostSearching => picker.visual_remote(FOLDER_QUERY, Remote::searching(), cx),
            Scene::SshHostDiscovering => picker.visual_remote(FOLDER_QUERY, discovering(), cx),
            _ => picker.visual_remote(
                FOLDER_QUERY,
                Remote::failed("Permission denied (publickey,password)."),
                cx,
            ),
        }
    });
}

fn listing() -> Remote {
    Remote::ready(
        "/home/alex",
        &[
            ("/home/alex/src/xenon", true),
            ("/home/alex/src/xenon-site", true),
            ("/home/alex/src/xterm-notes", false),
        ],
        &[
            ("/home/alex/dev/xen-notes", false),
            ("/home/alex/Projects/xenial", true),
        ],
    )
}

/// The listing is on screen; the name search is still running.
fn discovering() -> Remote {
    Remote::discovering(
        "/home/alex",
        &[
            ("/home/alex/src/xenon", true),
            ("/home/alex/src/xenon-site", true),
            ("/home/alex/src/xterm-notes", false),
        ],
    )
}
