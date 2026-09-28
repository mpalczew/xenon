// Keyboard regressions that need a real window: focus must survive transient
// surfaces closing, or every app shortcut goes dead.

/// Worklist: Return opens the inline editor, Escape closes it, then ⌘⇧P
/// must still reach the app.
pub(crate) fn shortcuts_survive_worklist_escape(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    use anyhow::{anyhow, ensure};

    let window = crate::open_window(cx)?;
    window.update(cx, |app, window, cx| {
        xenon_ui::apply_scene(app, xenon_ui::Scene::WorklistEmpty, window, cx);
    })?;
    cx.run_until_parked();
    window.update(cx, |app, window, cx| app.visual_focus_editor(window, cx))?;
    cx.run_until_parked();
    for keys in ["enter", "escape", "cmd-shift-p"] {
        cx.simulate_keystrokes(window.into(), keys);
        cx.update_window(window.into(), |_, window, _| window.refresh())?;
        cx.run_until_parked();
    }
    let open = window.update(cx, |app, _, _| app.visual_command_palette_open())?;
    ensure!(open, "{}", anyhow!("⌘⇧P did nothing after closing the worklist editor"));
    Ok(())
}
