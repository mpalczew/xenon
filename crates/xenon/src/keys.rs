//! The key map: one table of GTK action -> accelerators, and the text shown for
//! a shortcut. The focused terminal keeps Ctrl+C, X, B, E, O and S for the
//! shell, so the table has no accelerator for them.

use gtk::prelude::*;

/// GTK action name -> accelerators. The first one is the one shown in the UI.
const BINDINGS: &[(&str, &[&str])] = &[
    ("app.new-terminal", &["<Control>n"]),
    ("app.new-workspace", &["<Control><Alt>n"]),
    ("app.open-workspace", &["<Control><Shift>o"]),
    ("app.open-finder", &["<Control>p"]),
    ("app.command-palette", &["<Control><Shift>p"]),
    ("app.run-task", &["<Control><Shift>r"]),
    ("app.capture-worklist", &["<Control><Shift>k"]),
    ("app.open-worklist", &["<Control><Alt>k"]),
    ("app.undo-capture", &["<Control><Alt>z"]),
    ("app.run-build-task", &["<Control><Shift>b"]),
    ("app.allow-folder-tasks", &["<Control><Alt>Return"]),
    ("app.deny-folder-tasks", &["<Control><Alt>BackSpace"]),
    ("app.close-tab", &["<Control>w"]),
    ("app.close-other-tabs", &["<Control><Alt><Shift>w"]),
    (
        "app.next-tab",
        &[
            "<Control>Tab",
            "<Control>braceright",
            "<Control><Shift>bracketright",
        ],
    ),
    (
        "app.prev-tab",
        &[
            "<Control><Shift>Tab",
            "<Control>braceleft",
            "<Control><Shift>bracketleft",
        ],
    ),
    ("app.next-pane", &["<Control>grave"]),
    ("app.next-workspace", &["<Control><Alt>Down"]),
    ("app.prev-workspace", &["<Control><Alt>Up"]),
    ("app.close-workspace", &["<Control><Alt>w"]),
    ("app.split-right", &["<Control>backslash"]),
    (
        "app.split-down",
        &["<Control>bar", "<Control><Shift>backslash"],
    ),
    ("app.park-pane", &["<Control><Alt>backslash"]),
    ("app.focus-terminal", &["<Control>1"]),
    ("app.focus-sidebar", &["<Control>3"]),
    ("app.focus-git", &["<Control><Alt>g"]),
    ("app.settings", &["<Control>comma"]),
    ("app.cycle-diff-layout", &["<Control><Alt><Shift>d"]),
    ("app.toggle-sidebar", &["<Control><Alt>b"]),
    ("app.find", &["<Control><Shift>f"]),
    ("app.find-next", &["F3"]),
    ("app.find-prev", &["<Shift>F3"]),
    ("app.copy", &["<Control><Shift>c"]),
    ("app.copy-clean", &["<Control><Shift>y"]),
    ("app.paste", &["<Control>v", "<Control><Shift>v"]),
    ("app.zoom-in", &["<Control>equal", "<Control>plus"]),
    ("app.zoom-out", &["<Control>minus"]),
    ("app.zoom-reset", &["<Control>0"]),
    ("app.connect-phone", &["<Control><Shift>m"]),
    ("app.quit", &["<Control>q"]),
];

/// Register every accelerator on the application.
pub(crate) fn install(app: &impl IsA<gtk::Application>) {
    for (action, accels) in BINDINGS {
        app.set_accels_for_action(action, accels);
    }
}

/// An accelerator as text: `<Control><Shift>p` -> `Ctrl+Shift+P`.
pub(crate) fn label(accel: &str) -> String {
    match gtk::accelerator_parse(accel) {
        Some((key, mods)) => gtk::accelerator_get_label(key, mods).to_string(),
        None => accel.to_string(),
    }
}

/// The first key bound to `action`, as text.
pub(crate) fn action_label(action: &str) -> Option<String> {
    let (_, accels) = BINDINGS.iter().find(|(name, _)| *name == action)?;
    accels.first().map(|accel| label(accel))
}
