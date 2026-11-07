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
