//! Visual regression runner: Metal `render_to_image` of shipped Xenon views.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Visual test runner is only supported on macOS");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
mod compare {
    include!("visual_test_runner/compare.rs");
}

#[cfg(target_os = "macos")]
mod fixtures {
    include!("visual_test_runner/fixtures.rs");
}

#[cfg(target_os = "macos")]
mod remote {
    include!("visual_test_runner/remote.rs");
}

#[cfg(target_os = "macos")]
mod review {
    include!("visual_test_runner/review.rs");
}

#[cfg(target_os = "macos")]
fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    if let Err(error) = run_tests() {
        eprintln!("Visual tests failed: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
fn run_tests() -> anyhow::Result<()> {
    use anyhow::anyhow;
    use xenon_ui::surface_names;

    let update = std::env::var("UPDATE_BASELINE").is_ok();
    let output_dir = output_dir();
    std::fs::create_dir_all(&output_dir)?;
    let baseline_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_fixtures/visual_tests");
    std::fs::create_dir_all(&baseline_dir)?;

    let _fixture = fixtures::build()?;
    let mut cx = gpui::VisualTestAppContext::new(gpui_platform::current_platform(false));
    cx.update(|cx| {
        xenon_terminal::init(cx);
        xenon_ui::init(cx);
    });

    let mut captured = Vec::new();
    let mut failed = Vec::new();
    for scene in xenon_ui::SCENES {
        match run_scene(*scene, &mut cx, &output_dir, &baseline_dir, update) {
            Ok(name) => {
                println!("ok {name}");
                captured.push(name.to_string());
            }
            Err(error) => {
                eprintln!("FAIL {}: {error:#}", scene.name());
                failed.push(scene.name().to_string());
            }
        }
    }

    match remote::capture(&output_dir) {
        Ok(names) => {
            for name in names {
                let png = output_dir.join(format!("{name}.png"));
                let actual = image::open(&png)?.to_rgba8();
                if let Err(error) = compare::assert_filled(&actual, &name) {
                    eprintln!("FAIL {name}: {error:#}");
                    failed.push(name);
                    continue;
                }
                if let Err(error) = compare::compare_or_update(
                    &actual,
                    &baseline_dir.join(format!("{name}.png")),
                    &png,
                    update,
                ) {
                    eprintln!("FAIL {name}: {error:#}");
                    failed.push(name);
                } else {
                    println!("ok {name}");
                    captured.push(name);
                }
            }
        }
        Err(error) => {
            eprintln!("FAIL remote html: {error:#}");
            failed.push("remote_auth".into());
            failed.push("remote_session".into());
        }
    }

    let mut names: Vec<&str> = surface_names();
    names.sort_unstable();
    captured.sort();
    let missing: Vec<_> = names
        .iter()
        .copied()
        .filter(|name| captured.iter().all(|got| got != name))
        .collect();
    if !missing.is_empty() {
        failed.push("inventory".into());
        eprintln!("missing surfaces: {missing:?}");
    }

    let captured_refs: Vec<&str> = captured.iter().map(String::as_str).collect();
    let review = review::write_page(&output_dir, &captured_refs, "")?;
    println!("review page {}", review.display());
    if update {
        let committed =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../visual-review");
        review::write_page(
            &committed,
            &names,
            "../crates/xenon/test_fixtures/visual_tests/",
        )?;
    }

    if failed.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("{} visual tests failed: {failed:?}", failed.len()))
    }
}

#[cfg(target_os = "macos")]
fn run_scene(
    scene: xenon_ui::Scene,
    cx: &mut gpui::VisualTestAppContext,
    output_dir: &std::path::Path,
    baseline_dir: &std::path::Path,
    update: bool,
) -> anyhow::Result<&'static str> {
    use anyhow::anyhow;
    use gpui::{Modifiers, px};
    use std::time::Duration;
    use xenon_ui::apply_scene;

    let name = scene.name();
    let window = open_window(cx)?;
    window.update(cx, |app, window, cx| {
        apply_scene(app, scene, window, cx);
    })?;
    cx.run_until_parked();
    if scene.needs_terminal() {
        wait_for_terminal(cx, window)?;
    }
    if scene == xenon_ui::Scene::TabTooltip {
        cx.simulate_mouse_move(
            window.into(),
            gpui::point(px(255.0), px(50.0)),
            None,
            Modifiers::none(),
        );
        cx.advance_clock(Duration::from_millis(700));
        cx.run_until_parked();
    }
    if scene == xenon_ui::Scene::WorkspacePicker {
        cx.simulate_input(window.into(), "archived");
        cx.advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
    }
    if scene == xenon_ui::Scene::SidebarWorking {
        cx.advance_clock(Duration::from_millis(700));
        cx.run_until_parked();
    }
    if scene.needs_terminal() {
        std::thread::sleep(Duration::from_millis(150));
        cx.run_until_parked();
    }
    cx.update_window(window.into(), |_, window, _| window.refresh())?;
    cx.run_until_parked();

    let image = if matches!(
        scene,
        xenon_ui::Scene::SettingsWindow | xenon_ui::Scene::SettingsDropdown
    ) {
        let settings = window
            .update(cx, |app, _, _| app.visual_settings_window())?
            .ok_or_else(|| anyhow!("{name}: settings window missing"))?;
        cx.update_window(settings.into(), |_, window, _| window.refresh())?;
        cx.run_until_parked();
        cx.capture_screenshot(settings.into())?
    } else {
        cx.capture_screenshot(window.into())?
    };
    compare::assert_filled(&image, name)?;
    compare::compare_or_update(
        &image,
        &baseline_dir.join(format!("{name}.png")),
        &output_dir.join(format!("{name}.png")),
        update,
    )?;
    Ok(name)
}

#[cfg(target_os = "macos")]
fn open_window(
    cx: &mut gpui::VisualTestAppContext,
) -> anyhow::Result<gpui::WindowHandle<xenon_ui::XenonApp>> {
    use gpui::{AppContext as _, px, size};
    cx.open_offscreen_window(size(px(1280.0), px(800.0)), |_window, cx| {
        cx.new(xenon_ui::XenonApp::new_visual)
    })
}

#[cfg(target_os = "macos")]
fn wait_for_terminal(
    cx: &mut gpui::VisualTestAppContext,
    window: gpui::WindowHandle<xenon_ui::XenonApp>,
) -> anyhow::Result<()> {
    use anyhow::anyhow;
    use std::time::Duration;
    for _ in 0..80 {
        if window.update(cx, |app, _, cx| app.visual_terminal_ready(cx))? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
    Err(anyhow!("terminal did not become ready"))
}

#[cfg(target_os = "macos")]
fn output_dir() -> std::path::PathBuf {
    std::env::var("VISUAL_TEST_OUTPUT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("target/visual_tests"))
}
