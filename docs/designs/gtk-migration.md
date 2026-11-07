# GTK migration: remaining work

The GTK app (`crates/xenon_gtk`) is the app. The GPUI crates stay only as the reference
for text and behaviour until they are deleted. The task list is `.xenon/worklist.md`;
this page holds the rules for porting and the gaps in detail.

## Porting rule

Copy user-visible text from the GPUI app word for word and in the same order: labels,
hints, tooltips, empty states and settings text. Never reword, invent or drop any of it.
When both frontends show a string, move it into a GPUI-free crate so both read the same
one. Text for a GTK-only control is new; say so in the report.

Prefer GTK, libadwaita, VTE and GtkSourceView built-ins over our own widgets, drawing
and logic, even where the look differs from the GPUI app. Fewer lines beat a pixel
match. Write our own only when no built-in does the job, and say why in the report.

## Shared crates

Rules both frontends use live in GPUI-free crates, so the GTK code only draws:
`xenon_core` (workspaces, pane tree, attention and working status, titles, tasks),
`xenon_store` (files under the state dir, CLI parsing, the `xenon open` socket
protocol), `xenon_settings`, `xenon_git`, `xenon_finder`, `xenon_keymap`,
`xenon_remote`, `xenon_prose`. `xenon_lsp` waits for its GTK port.

## Gaps

- **Toolbar.** Not drawn yet: Go Back and Go Forward, Cut, Toggle File Tree, the two
  worklist buttons, Run Task, Save (shown when an editor is dirty).
- **Empty screen.** The Command Palette, Focus Editor and Keyboard Shortcuts rows are
  listed with their keys but do nothing.
- **Settings.** An `AdwPreferencesDialog` drawn inside the window: it can't be moved or
  resized on its own, and some pages are near empty while others scroll.
- **Sidebar.** The Git section has no draggable height.
- **Editor.** No LSP and no diff gutter. Themes still come from the bundled Zed themes
  instead of following the system.
- **Markdown prose mode.** No Paper surface, quote rule, round bullet glyphs, table grid,
  code highlighting or list continuation on Return (see DESIGN.md).
- **`xenon open`.** `--pane` and the column in `path:line:col` are ignored.
- **Missing features.** Command palette, divider resize from the keyboard, the worklist
  panel. The diff tab is not part of the saved layout (same as GPUI).
- **Phone remote.** Tasks, mouse clicks, dim text and OSC 8 links (VTE's HTML has no
  markup for them), cursor hiding, and the phone's own light/dark choice (the phone
  always gets the computer's terminal colors).

## Deleting GPUI

Delete `crates/xenon`, `xenon_ui`, `xenon_terminal`, `xenon_editor`,
`xenon_design_system`, `third_party/`, `xenon_ide` and `xenon_ssh` (not ported), the
`visual-tests` feature, and the code in shared crates that only GPUI uses (the diff
parser in `xenon_git`, the GPUI copy of `TerminalActivity`). Do it after the text the
GTK app still needs has been copied.
