use gpui::{Context, Window};

use super::{XenonApp, populate};

pub(super) fn task_input(
    app: &mut XenonApp,
    scene: super::Scene,
    window: &mut Window,
    cx: &mut Context<XenonApp>,
) {
    populate(app, window, cx);
    app.visual_task_input(scene, cx);
}

pub(super) fn task_trust(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.show_trust_prompt_fixture(cx);
}

/// A task's own tab: `demo-greet` runs `/bin/echo` as a process task.
pub(super) fn task_tab(app: &mut XenonApp, window: &mut Window, cx: &mut Context<XenonApp>) {
    populate(app, window, cx);
    app.visual_run_task("demo-greet", "Xenon", cx);
}
