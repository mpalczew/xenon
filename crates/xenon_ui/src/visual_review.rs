//! Copyable disapproval string for the screenshot review page.

/// Names of shots the user marked, in page order.
pub fn selected_names<'a>(items: impl IntoIterator<Item = (&'a str, bool)>) -> Vec<String> {
    items
        .into_iter()
        .filter(|(_, checked)| *checked)
        .map(|(name, _)| name.to_string())
        .collect()
}

/// Clipboard payload pasted back into chat. Empty when nothing is selected.
pub fn disapproval_clipboard(names: &[impl AsRef<str>]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let mut out = String::from("DISAPPROVED:\n");
    for name in names {
        out.push_str("- ");
        out.push_str(name.as_ref());
        out.push('\n');
    }
    out.pop();
    out
}

#[cfg(test)]
mod tests {
    use super::{disapproval_clipboard, selected_names};

    #[test]
    fn empty_selection_is_empty_string() {
        assert_eq!(disapproval_clipboard(&[] as &[&str]), "");
        assert!(selected_names([("a", false), ("b", false)]).is_empty());
    }

    #[test]
    fn selected_shots_become_copyable_list() {
        let names = selected_names([
            ("chrome_populated_dark", true),
            ("overlay_finder", false),
            ("settings_window", true),
        ]);
        assert_eq!(names, vec!["chrome_populated_dark", "settings_window"]);
        assert_eq!(
            disapproval_clipboard(&names),
            "DISAPPROVED:\n- chrome_populated_dark\n- settings_window"
        );
    }
}
