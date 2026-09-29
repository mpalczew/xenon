// Keyboard regressions that need a real window: focus must survive transient
// surfaces closing, or every app shortcut goes dead.

type Check = fn(&mut gpui::VisualTestAppContext) -> anyhow::Result<()>;

pub(crate) fn run_all(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    let checks: [(&str, Check); 4] = [
        ("keyboard_worklist_escape", shortcuts_survive_worklist_escape),
        ("keyboard_settings_rows", settings_keys_reach_rows),
        ("keyboard_offscreen_focus", shortcuts_survive_offscreen_focus),
        ("keyboard_stranded_key_replay", stranded_key_is_replayed),
    ];
    let mut failed = Vec::new();
    for (name, check) in checks {
        match check(cx) {
            Ok(()) => println!("ok {name}"),
            Err(error) => {
                eprintln!("FAIL {name}: {error:#}");
                failed.push(name);
            }
        }
    }
    anyhow::ensure!(failed.is_empty(), "keyboard checks failed: {failed:?}");
    Ok(())
}

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

/// Settings: ⌘3 opens Editor, arrows reach Wrap prose, Space flips it.
fn settings_keys_reach_rows(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    let window = crate::open_window(cx)?;
    window.update(cx, |app, window, cx| {
        xenon_ui::apply_scene(app, xenon_ui::Scene::SettingsWindow, window, cx);
    })?;
    cx.run_until_parked();
    let settings = window
        .update(cx, |app, _, _| app.visual_settings_window())?
        .ok_or_else(|| anyhow::anyhow!("settings window missing"))?;
    let before = cx.update(|cx| xenon_settings::wrap_prose(cx));
    for keys in ["cmd-3", "down", "down", "space"] {
        cx.simulate_keystrokes(settings.into(), keys);
        cx.update_window(settings.into(), |_, window, _| window.refresh())?;
        cx.run_until_parked();
    }
    let after = cx.update(|cx| xenon_settings::wrap_prose(cx));
    // Put it back for anything that runs later.
    cx.simulate_keystrokes(settings.into(), "space");
    cx.run_until_parked();
    anyhow::ensure!(after != before, "⌘3 ↓ ↓ Space did not toggle Wrap prose");
    Ok(())
}

/// Focus parked on a live handle that nothing renders: shortcuts still work.
pub(crate) fn shortcuts_survive_offscreen_focus(
    cx: &mut gpui::VisualTestAppContext,
) -> anyhow::Result<()> {
    palette_opens_after_offscreen_focus(cx, false)
}

/// Same, with only the keystroke fallback: the stranded ⌘⇧P is replayed.
pub(crate) fn stranded_key_is_replayed(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    palette_opens_after_offscreen_focus(cx, true)
}

fn palette_opens_after_offscreen_focus(
    cx: &mut gpui::VisualTestAppContext,
    keystroke_guard_only: bool,
) -> anyhow::Result<()> {
    let window = crate::open_window(cx)?;
    window.update(cx, |app, window, cx| {
        xenon_ui::apply_scene(app, xenon_ui::Scene::PopulatedDark, window, cx);
        if keystroke_guard_only {
            app.visual_keystroke_guard_only();
        }
    })?;
    cx.run_until_parked();
    let _offscreen = window.update(cx, |app, window, cx| app.visual_focus_offscreen(window, cx))?;
    cx.run_until_parked();
    cx.simulate_keystrokes(window.into(), "cmd-shift-p");
    cx.update_window(window.into(), |_, window, _| window.refresh())?;
    cx.run_until_parked();
    let open = window.update(cx, |app, _, _| app.visual_command_palette_open())?;
    anyhow::ensure!(open, "⌘⇧P did nothing while focus sat on an unrendered handle");
    Ok(())
}
