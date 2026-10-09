//! Workspace picker inside an SSH host, with fixed offline answers.

use gpui::{Context, Window};

use super::{Scene, XenonApp, overlays};
use crate::workspace_picker::VisualRemote as Remote;

const HOSTS: [&str; 3] = ["devbox", "pixelbook", "thinkpad"];

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
    let remote = match scene {
        Scene::SshHostResults => listing(),
        Scene::SshHostSearching => Remote::searching(),
        Scene::SshHostError => Remote::failed("Permission denied (publickey,password)."),
        _ => {
            picker.update(cx, |picker, cx| picker.visual_hosts(&HOSTS, cx));
            return;
        }
    };
    picker.update(cx, |picker, cx| {
        picker.visual_hosts(&HOSTS, cx);
        picker.visual_in_host("thinkpad", "xen", remote, cx)
    });
}

fn listing() -> Remote {
    Remote::ready(
        "/home/alex",
        &[
            ("/home/alex/src/xenon", true),
            ("/home/alex/src/xenon-site", true),
            ("/home/alex/src/xenon/crates/xenon_ui", false),
            ("/home/alex/dev/xen-notes", false),
            ("/home/alex/Projects/xenial", true),
        ],
    )
}
