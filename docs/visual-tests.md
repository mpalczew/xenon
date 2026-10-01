# Screenshot tests

If a surface paints, it belongs in this suite. UI work is incomplete until the
new pixels are captured and reviewed (`/ui-iteration`).

## Run

```bash
scripts/visual-tests
```

Writes current PNGs to `target/visual_tests/` and compares them to
`crates/xenon/test_fixtures/visual_tests/`. The suite fails on pixel divergence.

Results do not depend on the attached display: scenes fix window size, and the
runner pins `NSScreen.backingScaleFactor` to 2.0 for its process
(`visual_test_runner/display.rs`), so a 1x monitor renders Retina baselines.

Scenes for lists and scrolling panes use enough content to overflow the window.

Not part of `scripts/health` / `cargo test`: Metal capture needs a macOS window
server. Do not add this to GitHub macOS runners unless asked.

Remote HTML shots (`remote_pair`, `remote_code`, `remote_workspaces`,
`remote_session`, `remote_reconnecting`) render the phone page offline via its
`xenonFixture(name)` hook and need Google Chrome at
`/Applications/Google Chrome.app`. Missing Chrome fails those surfaces.
Headless Chrome lays out at least 500px wide, so fixtures pin the app to 390×844.

The suite also runs keyboard regressions that need a real window
(`visual_test_runner/keyboard.rs`). `VISUAL_TEST_ONLY=keyboard
scripts/visual-tests` runs just those.

## Update baselines

After an intentional visual change:

```bash
UPDATE_BASELINE=1 scripts/visual-tests
```

Then open `target/visual_tests/index.html` (or `visual-review/index.html`) and
look at the images. Passing tests without opening the shots does not count.

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
