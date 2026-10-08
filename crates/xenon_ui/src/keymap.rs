//! Installs the platform default keymap plus `~/.xenon/keymap.json`.
//!
//! Parsing and building actions is zed's `settings::KeymapFile`; the defaults
//! and display text come from the pure `xenon_keymap` crate. A bad user file
//! never replaces a working keymap.

use std::path::PathBuf;

use gpui::{App, Global, KeyBinding};
use settings::{KeymapFile, KeymapFileLoadResult};
use xenon_keymap::{Entry, Platform, STARTER_KEYMAP};

pub(crate) fn keymap_path() -> PathBuf {
    xenon_store::data_dir().join("keymap.json")
}

/// The user-file text the live keymap was built from (`None`: no file).
struct Applied(Option<String>);
impl Global for Applied {}

/// A user-file error found at startup, shown once the window exists.
pub(crate) struct StartupError(pub String);
impl Global for StartupError {}

struct User {
    bindings: Vec<KeyBinding>,
    entries: Vec<Entry>,
}

/// Startup: defaults always; the user file when it is valid.
pub(crate) fn install(cx: &mut App) {
    let text = read_user_file();
    let parsed = text.as_deref().map(|text| parse_user(text, cx));
    let (user, file_error) = match parsed {
        Some(Ok(user)) => (Some(user), None),
        Some(Err(message)) => (None, Some(message)),
        None => (None, None),
    };
    let built = build(cx, user);
    cx.set_global(Applied(text));
    if let Some(message) = file_error.or(built.err()) {
        log::warn!("keymap: {message}");
        cx.set_global(StartupError(message));
    }
}

/// File changed: rebuild. `Ok(false)` when nothing differs from what is live.
pub(crate) fn reload(cx: &mut App) -> Result<bool, String> {
    let text = read_user_file();
    if cx.try_global::<Applied>().is_some_and(|a| a.0 == text) {
        return Ok(false);
    }
    let user = text
        .as_deref()
        .map(|text| parse_user(text, cx))
        .transpose()?;
    build(cx, user)?;
    cx.set_global(Applied(text));
    Ok(true)
}

fn read_user_file() -> Option<String> {
    std::fs::read_to_string(keymap_path()).ok()
}

fn build(cx: &mut App, user: Option<User>) -> Result<(), String> {
    let platform = Platform::current();
    let defaults = xenon_keymap::defaults(platform);
    let mut bindings = load(&xenon_keymap::to_json(&defaults), cx)?;
    let entries = match user {
        Some(user) => {
            bindings.extend(user.bindings);
            user.entries
        }
        None => Vec::new(),
    };
    cx.clear_key_bindings();
    cx.bind_keys(bindings);
    xenon_keymap::set_display_overrides(xenon_keymap::display_overrides(platform, &entries));
    Ok(())
}

fn parse_user(text: &str, cx: &App) -> Result<User, String> {
    let bindings = load(text, cx)?;
    let file = KeymapFile::parse(text).map_err(|error| error.to_string())?;
    Ok(User {
        bindings,
        entries: entries_of(&file),
    })
}

fn entries_of(file: &KeymapFile) -> Vec<Entry> {
    let mut entries = Vec::new();
    for section in file.sections() {
        let context = (!section.context.is_empty()).then_some(section.context.as_str());
        for (keystroke, action) in section.bindings() {
            if let Ok(parsed) = KeymapFile::parse_action(action) {
                let name = parsed.map(|(name, _input)| name.as_str());
                entries.push(Entry::new(context, keystroke, name));
            }
        }
    }
    entries
}

fn load(text: &str, cx: &App) -> Result<Vec<KeyBinding>, String> {
    match KeymapFile::load(text, cx) {
        KeymapFileLoadResult::Success { key_bindings } => Ok(key_bindings),
        KeymapFileLoadResult::SomeFailedToLoad { error_message, .. } => {
            Err(summarize(&error_message.0))
        }
        KeymapFileLoadResult::JsonParseFailure { error } => Err(first_line(&error.to_string())),
    }
}

/// zed's multi-line markdown report, cut to the first failing binding.
fn summarize(report: &str) -> String {
    let first = report
        .lines()
        .find_map(|line| line.trim().strip_prefix("- In "))
        .or_else(|| {
            report
                .lines()
                .find(|line| line.trim().starts_with("Parse error"))
        })
        .unwrap_or(report);
    first_line(&first.replace('`', ""))
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).trim().to_owned()
}

/// Create the file with a valid empty keymap when it does not exist yet.
pub(crate) fn ensure_file() -> std::io::Result<PathBuf> {
    let path = keymap_path();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, STARTER_KEYMAP)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_keeps_the_first_failing_binding() {
        let report = "Errors in user keymap file.\nIn section without context predicate:\n\n- In binding `\"ctrl-x\"`, didn't find an action named \"xenon::Nope\".\n\n- In binding `\"ctrl-y\"`, other";
        assert_eq!(
            summarize(report),
            "binding \"ctrl-x\", didn't find an action named \"xenon::Nope\"."
        );
    }

    fn with_app<R>(run: impl FnOnce(&mut App) -> R) -> R {
        let cx = gpui::TestAppContext::single();
        cx.update(run)
    }

    #[test]
    fn both_platform_defaults_load() {
        for platform in [Platform::Mac, Platform::Linux] {
            let json = xenon_keymap::to_json(&xenon_keymap::defaults(platform));
            let count = xenon_keymap::defaults(platform).len();
            let loaded = with_app(|cx| load(&json, cx));
            assert_eq!(loaded.map(|b| b.len()), Ok(count), "{platform:?}");
        }
    }

    #[test]
    fn user_file_parses_bindings_and_null() {
        let text = r#"[{"bindings": {"ctrl-alt-p": "xenon::FilePalette"}},
                       {"context": "Terminal", "bindings": {"ctrl-x": null}}]"#;
        let user = with_app(|cx| parse_user(text, cx)).unwrap();
        assert_eq!(user.bindings.len(), 2);
        assert_eq!(
            user.entries,
            vec![
                Entry::new(None, "ctrl-alt-p", Some("xenon::FilePalette")),
                Entry::new(Some("Terminal"), "ctrl-x", None),
            ]
        );
    }

    #[test]
    fn unknown_action_is_an_error() {
        let text = r#"[{"bindings": {"ctrl-x": "xenon::NoSuchThing"}}]"#;
        let error = with_app(|cx| parse_user(text, cx)).err().unwrap();
        assert!(error.contains("xenon::NoSuchThing"), "{error}");
    }

    #[test]
    fn bad_keystroke_is_an_error() {
        let text = r#"[{"bindings": {"ctrl-shift-enter-x": "xenon::Save"}}]"#;
        assert!(with_app(|cx| parse_user(text, cx)).is_err());
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(with_app(|cx| parse_user("[{", cx)).is_err());
    }

    #[test]
    fn comments_and_empty_files_are_fine() {
        let text = "// mine\n[]";
        assert!(with_app(|cx| parse_user(text, cx)).is_ok());
        assert!(with_app(|cx| parse_user("", cx)).is_ok());
        assert!(with_app(|cx| parse_user(STARTER_KEYMAP, cx)).is_ok());
    }

    #[test]
    fn user_null_unbinds_a_default_and_user_wins_on_conflict() {
        let open_file = if cfg!(target_os = "macos") {
            "cmd-p"
        } else {
            "ctrl-p"
        };
        let text = format!(r#"[{{"bindings": {{"{open_file}": null, "alt-9": "xenon::Save"}}}}]"#);
        let (unbound, rebound) = with_app(|cx| {
            let user = parse_user(&text, cx).unwrap();
            build(cx, Some(user)).unwrap();
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let press = |key: &str| {
                let stroke = gpui::Keystroke::parse(key).unwrap();
                keymap.bindings_for_input(&[stroke], &[]).0.len()
            };
            xenon_keymap::set_display_overrides(Default::default());
            (press(open_file), press("alt-9"))
        });
        assert_eq!((unbound, rebound), (0, 1));
    }

    #[test]
    fn linux_terminal_passes_shell_keys_and_copies_with_shift() {
        let json = xenon_keymap::to_json(&xenon_keymap::defaults(Platform::Linux));
        let actions_for = |key: &str, contexts: &str| {
            with_app(|cx| {
                cx.bind_keys(load(&json, cx).unwrap());
                let stack = vec![
                    gpui::KeyContext::parse("XenonApp").unwrap(),
                    gpui::KeyContext::parse(contexts).unwrap(),
                ];
                let stroke = gpui::Keystroke::parse(key).unwrap();
                let keymap = cx.key_bindings();
                let (found, _) = keymap.borrow().bindings_for_input(&[stroke], &stack);
                found
                    .iter()
                    .map(|b| b.action().name().to_owned())
                    .collect::<Vec<_>>()
            })
        };
        for key in [
            "ctrl-c", "ctrl-b", "ctrl-e", "ctrl-o", "ctrl-s", "ctrl-x", "ctrl-v",
        ] {
            assert!(
                actions_for(key, "Terminal").is_empty(),
                "{key} must reach the PTY"
            );
        }
        assert_eq!(actions_for("ctrl-c", "Editor"), ["xenon_clipboard::Copy"]);
        assert_eq!(
            actions_for("ctrl-shift-c", "Terminal"),
            ["xenon_clipboard::Copy"]
        );
        assert_eq!(
            actions_for("ctrl-shift-v", "Terminal")[0],
            "xenon_clipboard::Paste"
        );
        assert_eq!(actions_for("ctrl-p", "Terminal"), ["xenon::FilePalette"]);
        assert_eq!(actions_for("ctrl-w", "Terminal"), ["xenon::CloseEditor"]);
    }
}
