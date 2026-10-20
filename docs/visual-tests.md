# Screenshot tests

Capture-only. The runner renders scenes so you can look at them and check the UI
matches its mock; there are no committed baselines and no pixel comparison. It
fails only when a scene crashes, fails to render, or paints blank.

Rules: at most one scene per unique UI state (no theme or data variants unless
that is the point). UI work is done when you have captured the scenes for the
surfaces you touched and looked at them (`/ui-iteration`).

## Run

```bash
scripts/visual-tests                                  # every scene
VISUAL_TEST_ONLY=overlay_finder scripts/visual-tests   # one scene
VISUAL_TEST_ONLY=overlay_ssh,content_worklist scripts/visual-tests   # prefixes
VISUAL_TEST_ONLY=keyboard scripts/visual-tests         # keyboard regressions only
```

`VISUAL_TEST_ONLY` takes comma-separated names or name prefixes. PNGs and
`index.html` land in `target/visual_tests/`. Scene list: `Scene` in
`crates/xenon_ui/src/app/visual/mod.rs`.

Results do not depend on the attached display: scenes fix window size, and the
runner pins `NSScreen.backingScaleFactor` to 2.0 for its process
(`visual_test_runner/display.rs`), so a 1x monitor renders Retina images.

Scenes for lists and scrolling panes use enough content to overflow the window.

Not part of `scripts/health` / `cargo test`: Metal capture needs a macOS window
server. Do not add this to GitHub macOS runners unless asked.

Git scenes (`app/visual/git.rs`) use fixed status and diff text, never a live
repo: `chrome_sidebar_markers[_light]` (every marker combination: needs-you +
dirty, working + index-only, idle + dirty, clean, selected), `chrome_git_*`
(expanded, expanded light, collapsed, clean, keyboard cursor), and
`content_diff_*` (split, split light, unified, narrow Auto, index-only
staged/unstaged). Workspace agent status is pinned per row with
`services.visual_status`.

Remote HTML shots (`remote_*`) render the phone page offline via its
`xenonFixture(name)` hook and need Google Chrome at
`/Applications/Google Chrome.app`. Missing Chrome fails those surfaces.
Headless Chrome lays out at least 500px wide, so fixtures pin the app to 390×844.

The keyboard regressions (`visual_test_runner/keyboard.rs`) are behavior checks
that need a real window; they run with the full suite and fail it.

## Review page

The generated page lists every shot. Check the ones you dislike and click
**Copy disapproved**. That puts a pasteable list on the clipboard / in the
textarea:

```
DISAPPROVED:
- chrome_populated_dark
- overlay_finder
```

Open in the everyday Chrome profile (not a throwaway automation profile):

```bash
open -na "Google Chrome" --args --profile-directory=Default "file://$PWD/target/visual_tests/index.html"
```
