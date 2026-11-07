# GTK app: rules and remaining gaps

The GTK app (`crates/xenon_gtk`) is the only app. The task list is `.xenon/worklist.md`;
this page holds the building rules and the gaps in detail.

## Rules

Prefer GTK, libadwaita, VTE and GtkSourceView built-ins over our own widgets, drawing
and logic, even where the look differs from the old GPUI app. Fewer lines beat a pixel
match. Write our own only when no built-in does the job, and say why in the report.

User-visible text that is shared between features lives in a GTK-free crate. Text for a
new control is new; say so in the report.

## Shared crates

Rules live in GTK-free crates, so the GTK code only draws: `xenon_core` (workspaces,
pane tree, attention and working status, titles, tasks), `xenon_store` (files under the
state dir, CLI parsing, the `xenon open` socket protocol), `xenon_settings`,
`xenon_git`, `xenon_finder`, `xenon_keymap`, `xenon_remote`, `xenon_prose`.

## Gaps

- **Toolbar.** Go Back, Go Forward, Cut and Toggle File Tree are not drawn.
- **Empty screen.** The Command Palette, Focus Editor and Keyboard Shortcuts rows are
  listed with their keys but do nothing.
- **Sidebar.** The Git section has no draggable height.
- **Editor.** No language servers and no diff gutter.
- **Markdown prose mode.** No Paper surface, quote rule, round bullet glyphs, table
  grid, code highlighting or list continuation on Return (see DESIGN.md).
- **`xenon open`.** `--pane` and the column in `path:line:col` are ignored.
- **Missing features.** Command palette, divider resize from the keyboard. The diff tab
  is not part of the saved layout.
- **Phone remote.** Tasks, mouse clicks, dim text and OSC 8 links (VTE's HTML has no
  markup for them), cursor hiding, and the phone's own light/dark choice (the phone
  always gets the computer's terminal colors).
