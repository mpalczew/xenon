use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use xenon_core::{Active, Registry, WorkspaceRec};
use xenon_store::{AppSettings, LspSettings, ThemeMode};

pub struct Fixture {
    _root: PathBuf,
}

pub fn build() -> Result<Fixture> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/visual_fixtures");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let data_dir = root.join("xenon-data");
    let demo_root = root.join("demo");
    let agents_root = root.join("agents");
    let archived_root = root.join("archived");
    fs::create_dir_all(data_dir.join("sessions"))?;
    fs::create_dir_all(demo_root.join("src"))?;
    fs::create_dir_all(demo_root.join(".vscode"))?;
    fs::create_dir_all(&agents_root)?;
    fs::create_dir_all(&archived_root)?;

    fs::write(
        demo_root.join("src/main.rs"),
        "fn main() {\n    let name = \"xenon\";\n    println!(\"hello {name}\");\n}\n",
    )?;
    fs::write(
        demo_root.join("NOTES.md"),
        "# Notes\n\n- terminal first\n- `cmd-p` opens files\n\n```rust\nfn ok() {}\n```\n",
    )?;
    for (name, body) in [
        ("src/settings.rs", "pub struct Settings;\n"),
        ("src/boot.rs", "pub fn boot() {}\n"),
        ("src/gemini.rs", "pub fn gemini() {}\n"),
        ("src/grok.rs", "pub fn grok() {}\n"),
        ("src/login.rs", "pub fn login() {}\n"),
        ("src/panels.rs", "pub fn panels() {}\n"),
        ("src/agent.rs", "pub fn agent() {}\n"),
        ("src/claude.rs", "pub fn claude() {}\n"),
        ("src/xenon.rs", "pub fn xenon() {}\n"),
        ("src/session.rs", "pub fn session() {}\n"),
        ("src/content.rs", "pub fn content() {}\n"),
        ("src/keyboard.rs", "pub fn keyboard() {}\n"),
    ] {
        fs::write(demo_root.join(name), body)?;
    }
    fs::write(demo_root.join("blob.bin"), [0u8, 1, 2, 3, 0xff, 0x00])?;
    fs::write(
        demo_root.join(".vscode/tasks.json"),
        r#"{
  "version": "2.0.0",
  "tasks": [
    { "label": "demo-build", "type": "shell", "command": "echo build" },
    { "label": "demo-test", "type": "shell", "command": "echo test" }
  ]
}"#,
    )?;
    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../macos/xenon-icon-source.png");
    fs::copy(icon, demo_root.join("logo.png"))?;
    fs::write(agents_root.join("README.md"), "agents workspace\n")?;
    fs::write(archived_root.join("README.md"), "archived\n")?;

    let shell = root.join("visual-shell");
    fs::write(
        &shell,
        "#!/bin/bash\nprintf '\\033[2J\\033[H'\nprintf '%s\\n' 'xenon visual fixture' 'fn main() { println!(\"hello\"); }' '$ '\nexec cat\n",
    )?;
    let mut perms = fs::metadata(&shell)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&shell, perms)?;

    let mut registry = Registry::default();
    let demo = named_workspace("demo", demo_root.clone());
    let agents = named_workspace("agents", agents_root);
    let archived = named_workspace("archived", archived_root);
    registry.active = Some(Active {
        workspace: demo.id,
    });
    registry.workspaces.push(demo);
    registry.workspaces.push(agents);
    registry.closed_workspaces.push(archived);

    write_json(&data_dir.join("workspaces.json"), &registry)?;
    let settings = AppSettings {
        theme: ThemeMode::Dark,
        lsp: LspSettings {
            enabled: false,
            ..LspSettings::default()
        },
        ..AppSettings::default()
    };
    write_json(&data_dir.join("settings.json"), &settings)?;

    unsafe {
        std::env::set_var("XENON_DATA_DIR", &data_dir);
        std::env::set_var("XENON_SKILL_HOME", &data_dir);
        std::env::remove_var("XERO_DATA_DIR");
        std::env::set_var("SHELL", &shell);
        std::env::set_var("TERM", "xterm-256color");
        std::env::set_var("PS1", "$ ");
    }

    Ok(Fixture { _root: root })
}

fn named_workspace(name: &str, root: PathBuf) -> WorkspaceRec {
    let mut rec = WorkspaceRec::new(root);
    rec.name = name.to_string();
    rec
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
