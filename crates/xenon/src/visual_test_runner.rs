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
mod display {
    include!("visual_test_runner/display.rs");
}

#[cfg(target_os = "macos")]
mod keyboard {
    include!("visual_test_runner/keyboard.rs");
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

    let output_dir = output_dir();
    std::fs::create_dir_all(&output_dir)?;
    let only = Only::from_env();

    display::pin_scale_factor()?;
    let _fixture = fixtures::build()?;
    let mut cx = gpui::VisualTestAppContext::new(gpui_platform::current_platform(false));
    cx.update(|cx| {
        xenon_terminal::init(cx);
        xenon_ui::init(cx);
    });

    let mut captured: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    for scene in xenon_ui::SCENES {
        if !only.wants(scene.name()) {
            continue;
        }
        match run_scene(*scene, &mut cx, &output_dir) {
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

    if only.wants("keyboard")
        && let Err(error) = keyboard::run_all(&mut cx)
    {
        eprintln!("FAIL keyboard: {error:#}");
        failed.push("keyboard".into());
    }

    if remote::FIXTURES
        .iter()
        .any(|f| only.wants(&format!("remote_{f}")))
    {
        capture_remote(&only, &output_dir, &mut captured, &mut failed);
    }

    captured.sort();
    let captured_refs: Vec<&str> = captured.iter().map(String::as_str).collect();
    let review = review::write_page(&output_dir, &captured_refs, "")?;
    println!("review page {}", review.display());

    if failed.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("{} visual tests failed: {failed:?}", failed.len()))
    }
}

/// `VISUAL_TEST_ONLY=a,b` limits the run to scenes whose name equals or starts
/// with a token (`keyboard` selects the keyboard checks). Unset runs everything.
#[cfg(target_os = "macos")]
struct Only(Option<Vec<String>>);

#[cfg(target_os = "macos")]
impl Only {
    fn from_env() -> Self {
        Self(std::env::var("VISUAL_TEST_ONLY").ok().map(|raw| {
            raw.split(',')
                .map(|token| token.trim().to_string())
                .filter(|token| !token.is_empty())
                .collect()
        }))
    }

    fn wants(&self, name: &str) -> bool {
        self.0
            .as_ref()
            .is_none_or(|tokens| tokens.iter().any(|token| name.starts_with(token.as_str())))
    }
}

#[cfg(target_os = "macos")]
fn capture_remote(
    only: &Only,
    output_dir: &std::path::Path,
    captured: &mut Vec<String>,
    failed: &mut Vec<String>,
) {
    match remote::capture(output_dir, |name| only.wants(name)) {
        Ok(names) => {
            for name in names {
                let png = output_dir.join(format!("{name}.png"));
                let filled = image::open(&png)
                    .map_err(anyhow::Error::from)
                    .and_then(|img| compare::assert_filled(&img.to_rgba8(), &name));
                match filled {
                    Ok(()) => {
                        println!("ok {name}");
                        captured.push(name);
                    }
                    Err(error) => {
                        eprintln!("FAIL {name}: {error:#}");
                        failed.push(name);
                    }
                }
            }
        }
        Err(error) => {
            eprintln!("FAIL remote html: {error:#}");
            failed.extend(remote::FIXTURES.iter().map(|f| format!("remote_{f}")));
        }
    }
}

#[cfg(target_os = "macos")]
fn run_scene(
    scene: xenon_ui::Scene,
    cx: &mut gpui::VisualTestAppContext,
    output_dir: &std::path::Path,
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
        std::thread::sleep(Duration::from_millis(200));
        cx.run_until_parked();
        window.update(cx, |app, _, cx| app.visual_after_terminal_ready(scene, cx))?;
        cx.run_until_parked();
    }
    if scene == xenon_ui::Scene::WorklistToolbarTooltip {
        cx.simulate_mouse_move(
            window.into(),
            gpui::point(px(1162.0), px(21.0)),
            None,
            Modifiers::none(),
        );
        cx.advance_clock(Duration::from_millis(700));
        cx.run_until_parked();
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
        xenon_ui::Scene::SettingsWindow
            | xenon_ui::Scene::SettingsDropdown
            | xenon_ui::Scene::SettingsAgentsInstalled
            | xenon_ui::Scene::SettingsRemote
            | xenon_ui::Scene::SettingsEditor
            | xenon_ui::Scene::SettingsLanguageServers
            | xenon_ui::Scene::SettingsSearch
    ) {
        let settings = window
            .update(cx, |app, _, _| app.visual_settings_window())?
            .ok_or_else(|| anyhow!("{name}: settings window missing"))?;
        cx.simulate_mouse_move(
            settings.into(),
            gpui::point(px(810.0), px(550.0)),
            None,
            Modifiers::none(),
        );
        cx.update_window(settings.into(), |_, window, _| window.refresh())?;
        cx.run_until_parked();
        cx.capture_screenshot(settings.into())?
    } else {
        cx.capture_screenshot(window.into())?
    };
    compare::assert_filled(&image, name)?;
    image.save(output_dir.join(format!("{name}.png")))?;
    Ok(name)
}

#[cfg(target_os = "macos")]
fn open_window(
    cx: &mut gpui::VisualTestAppContext,
) -> anyhow::Result<gpui::WindowHandle<xenon_ui::XenonApp>> {
    use gpui::{AppContext as _, px, size};
    cx.open_offscreen_window(size(px(1280.0), px(800.0)), |window, cx| {
        cx.new(|cx| xenon_ui::XenonApp::new_visual(window, cx))
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
