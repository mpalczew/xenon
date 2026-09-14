//! Drive the shipped review-page JS (not a reimplementation).

use std::process::Command;

fn review_js() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../visual-review/review.js")
}

#[test]
fn shipped_js_select_to_clipboard_string() {
    let Some(got) = eval_js(
        "disapprovalClipboard(selectedNames([\
            {name:'chrome_populated_dark', checked:true},\
            {name:'overlay_finder', checked:false},\
            {name:'settings_window', checked:true}\
        ]))",
    ) else {
        return;
    };
    assert_eq!(
        got,
        xenon_ui::disapproval_clipboard(&["chrome_populated_dark", "settings_window"])
    );
}

#[test]
fn shipped_js_empty_selection_is_empty() {
    let Some(got) = eval_js("disapprovalClipboard(selectedNames([{name:'a', checked:false}]))")
    else {
        return;
    };
    assert_eq!(got, "");
}

fn eval_js(expr: &str) -> Option<String> {
    let js = std::fs::read_to_string(review_js()).expect("visual-review/review.js");
    let script = format!("{js}\nprocess.stdout.write({expr} || '');\n");
    let output = Command::new("node").args(["-e", &script]).output().ok()?;
    assert!(
        output.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}
