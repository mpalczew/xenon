use anyhow::{Result, anyhow};
use std::fs;
use std::path::Path;
use std::process::Command;

const PAGE: &str = include_str!("../../../xenon_remote/src/page.html");

/// Phone page states, rendered offline through the page's `xenonFixture` hook.
pub const FIXTURES: &[&str] = &[
    "pair",
    "code",
    "workspaces",
    "session",
    "session_light",
    "reconnecting",
];

pub fn capture(output_dir: &Path) -> Result<Vec<String>> {
    fs::create_dir_all(output_dir)?;
    let mut names = Vec::new();
    for fixture in FIXTURES {
        let name = format!("remote_{fixture}");
        let html = output_dir.join(format!("{name}.html"));
        fs::write(&html, fixture_html(fixture))?;
        screenshot(&html, &output_dir.join(format!("{name}.png")), 390, 844)?;
        names.push(name);
    }
    Ok(names)
}

fn fixture_html(fixture: &str) -> String {
    format!("{PAGE}\n<script>xenonFixture('{fixture}')</script>\n")
}

fn screenshot(html: &Path, png: &Path, width: u32, height: u32) -> Result<()> {
    let chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    if !Path::new(chrome).exists() {
        return Err(anyhow!("Google Chrome not found at {chrome}"));
    }
    let user_dir = png.parent().unwrap().join(format!(
        "chrome-headless-{}",
        png.file_stem().unwrap_or_default().to_string_lossy()
    ));
    fs::create_dir_all(&user_dir)?;
    let _ = fs::remove_file(png);
    let url = format!("file://{}", html.canonicalize()?.display());
    let mut child = Command::new(chrome)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--disable-extensions",
            "--disable-sync",
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-default-apps",
            "--no-first-run",
            "--no-default-browser-check",
            "--virtual-time-budget=2000",
            &format!("--window-size={width},{height}"),
            &format!("--screenshot={}", png.display()),
            &format!("--user-data-dir={}", user_dir.display()),
            &url,
        ])
        .spawn()?;
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Err(anyhow!(
                    "chrome headless screenshot failed for {}",
                    html.display()
                ));
            }
            break;
        }
        if started.elapsed() > std::time::Duration::from_secs(12) {
            let _ = child.kill();
            let _ = child.wait();
            if png.exists() {
                return Ok(());
            }
            return Err(anyhow!("chrome headless timed out for {}", html.display()));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if !png.exists() {
        return Err(anyhow!("chrome did not write {}", png.display()));
    }
    Ok(())
}
