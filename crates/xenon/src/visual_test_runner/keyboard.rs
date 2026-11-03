type Check = fn(&mut gpui::VisualTestAppContext) -> anyhow::Result<()>;

pub(crate) fn run_all(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    let checks: [(&str, Check); 7] = [
        (
            "keyboard_worklist_escape",
            shortcuts_survive_worklist_escape,
        ),
        ("keyboard_settings_rows", settings_keys_reach_rows),
        (
            "keyboard_settings_search_typing",
            settings_typing_extends_search,
        ),
        (
            "keyboard_offscreen_focus",
            shortcuts_survive_offscreen_focus,
        ),
        ("keyboard_stranded_key_replay", stranded_key_is_replayed),
        ("keyboard_prose_vim", prose_takes_vim_keys),
        ("keyboard_prose_mode_scroll", mode_switch_keeps_scroll),
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
pub(crate) fn shortcuts_survive_worklist_escape(
    cx: &mut gpui::VisualTestAppContext,
) -> anyhow::Result<()> {
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
    ensure!(
        open,
        "{}",
        anyhow!("⌘⇧P did nothing after closing the worklist editor")
    );
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
    cx.simulate_keystrokes(settings.into(), "space");
    cx.run_until_parked();
    anyhow::ensure!(after != before, "⌘3 ↓ ↓ Space did not toggle Wrap prose");
    Ok(())
}

/// Settings: typing starts search, and later letters extend the query instead
/// of bubbling out of the field and restarting it.
fn settings_typing_extends_search(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    let window = crate::open_window(cx)?;
    window.update(cx, |app, window, cx| {
        xenon_ui::apply_scene(app, xenon_ui::Scene::SettingsWindow, window, cx);
    })?;
    cx.run_until_parked();
    let settings = window
        .update(cx, |app, _, _| app.visual_settings_window())?
        .ok_or_else(|| anyhow::anyhow!("settings window missing"))?;
    for keys in ["w", "r", "a", "p"] {
        cx.simulate_keystrokes(settings.into(), keys);
        cx.update_window(settings.into(), |_, window, _| window.refresh())?;
        cx.run_until_parked();
    }
    let query = settings.update(cx, |view, _, _| view.visual_query().to_owned())?;
    cx.simulate_keystrokes(settings.into(), "escape");
    cx.run_until_parked();
    anyhow::ensure!(query == "wrap", "typed \"wrap\" but search holds {query:?}");
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
    anyhow::ensure!(
        open,
        "⌘⇧P did nothing while focus sat on an unrendered handle"
    );
    Ok(())
}

/// Prose mode with vim on: search, `ciw` + typed text, Escape, `u`, and `j`
/// (one display row) all act on the buffer.
fn prose_takes_vim_keys(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    use anyhow::{anyhow, ensure};

    let window = crate::open_window(cx)?;
    window.update(cx, |app, window, cx| {
        xenon_ui::apply_scene(app, xenon_ui::Scene::ProseLight, window, cx);
    })?;
    cx.run_until_parked();
    window.update(cx, |app, window, cx| app.visual_focus_editor(window, cx))?;
    let vim_was = cx.update(|cx| xenon_settings::vim_mode(cx));
    if !vim_was {
        cx.update(xenon_settings::toggle_vim_mode);
    }
    let state = |cx: &mut gpui::VisualTestAppContext| {
        window
            .update(cx, |app, _, cx| app.visual_editor_state(cx))?
            .ok_or_else(|| anyhow!("no active editor"))
    };
    let press = |cx: &mut gpui::VisualTestAppContext, keys: &str| -> anyhow::Result<()> {
        for key in keys.split(' ') {
            cx.simulate_keystrokes(window.into(), key);
            cx.update_window(window.into(), |_, window, _| window.refresh())?;
            cx.run_until_parked();
        }
        Ok(())
    };
    let result = (|| {
        press(cx, "/ b r i e f enter c i w p l a n escape")?;
        let (text, _) = state(cx)?;
        ensure!(
            text.contains("Write the plan first"),
            "ciw in prose did not edit: {text:?}"
        );
        press(cx, "u")?;
        let (text, (row, _)) = state(cx)?;
        ensure!(
            text.contains("Write the brief first"),
            "u in prose did not undo"
        );
        press(cx, "j")?;
        let (_, (next, _)) = state(cx)?;
        ensure!(
            next == row + 1,
            "j moved from row {row} to {next}, wanted one row"
        );
        Ok(())
    })();
    if !vim_was {
        cx.update(xenon_settings::toggle_vim_mode);
    }
    result
}

/// Source ↔ prose keeps the reader on the same part of the file.
fn mode_switch_keeps_scroll(cx: &mut gpui::VisualTestAppContext) -> anyhow::Result<()> {
    use anyhow::{anyhow, ensure};
    use xenon_settings::MarkdownMode;

    let window = crate::open_window(cx)?;
    window.update(cx, |app, window, cx| {
        xenon_ui::apply_scene(app, xenon_ui::Scene::ProseLight, window, cx);
    })?;
    cx.run_until_parked();
    let editor = window
        .update(cx, |app, _, _| app.visual_editor())?
        .ok_or_else(|| anyhow!("no active editor"))?;
    let render = |cx: &mut gpui::VisualTestAppContext| -> anyhow::Result<()> {
        cx.update_window(window.into(), |_, window, _| window.refresh())?;
        cx.run_until_parked();
        Ok(())
    };
    let top = |cx: &mut gpui::VisualTestAppContext| {
        cx.update(|cx| editor.read(cx).visual_top_line())
            .ok_or_else(|| anyhow!("no top line"))
    };
    let filler: String = (1..=120)
        .map(|i| format!("## Part {i}\n\nParagraph {i} runs long enough to read as body text.\n\n"))
        .collect();
    let result = (|| {
        editor.update(cx, |e, cx| {
            e.set_markdown_mode(MarkdownMode::Source, cx);
            e.visual_insert(&filler, cx);
        });
        render(cx)?;
        editor.update(cx, |e, cx| e.visual_scroll_source(200, cx));
        render(cx)?;
        let source = top(cx)?;
        ensure!(source >= 150, "source did not scroll: top line {source}");
        editor.update(cx, |e, cx| e.set_markdown_mode(MarkdownMode::Prose, cx));
        render(cx)?;
        let prose = top(cx)?;
        ensure!(
            prose.abs_diff(source) <= 2,
            "prose opened at line {prose}, source was at {source}"
        );
        editor.update(cx, |e, cx| e.set_markdown_mode(MarkdownMode::Source, cx));
        render(cx)?;
        let back = top(cx)?;
        ensure!(
            back.abs_diff(prose) <= 2,
            "source came back at line {back}, prose was at {prose}"
        );
        Ok(())
    })();
    editor.update(cx, |e, cx| e.set_markdown_mode(MarkdownMode::Prose, cx));
    result
}
