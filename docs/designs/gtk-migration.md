# GTK migration map

A GTK 4 + VTE frontend is built beside the GPUI one in this repo. Both read and write
the same data files. GPUI is deleted at cutover. Step 0 (this list) makes the product
rules available without GPUI, so the new frontend only draws.

## Porting rule

Copy user-visible text from the GPUI app word for word and in the same order: labels,
hints, tooltips, empty states and settings text. Never reword, invent or drop any of it.
When both frontends show a string, move it into a GPUI-free crate so both read the same
one. Text for a GTK-only control is new; say so in the report.

Prefer GTK, libadwaita, VTE and GtkSourceView built-ins over our own widgets, drawing
and logic, even where the look differs from the GPUI app. Fewer lines beat a pixel
match. Write our own only when no built-in does the job, and say why in the report.

## Shared model crates

None of these depend on GPUI. A frontend uses them for the following.

- `xenon_core`: the data model and the rules on it. Workspaces and the registry,
  including which open workspace a CLI or agent file belongs to
  (`Registry::route_file`). The saved pane layout and the live pane tree (`PaneTree`,
  `PaneContent`: split, move tab, close tab, unsplit on last tab, parked empty panes,
  depth limits), generic over the frontend's own tab type. Terminal attention and
  working status (`AttentionMap`, `workspace_dot`, the agent-detection thresholds and
  `IDLE_AFTER`). Terminal title cleanup and tab labels. VS Code task parsing.
- `xenon_store`: reads and writes everything under `~/.xenon` (registry, sessions,
  `settings.json`), the CLI argument parser, and the local socket protocol that `xenon
  open` uses (`OpenFileSpec`, `OpenPane`).
- `xenon_settings`: app-wide settings as plain values (`UiSettings`): font faces and
  sizes with their clamps, wrap defaults, prose look, theme choice. It also holds the
  bundled font bytes and font-family aliasing. A frontend keeps one `UiSettings` in its
  own state, implements `FontProbe` over its text system, and calls `persist` after a
  change.
- `xenon_git`: git status for the sidebar (branch, changed files, line counts), and the
  read-only diff model (hunks, side-by-side rows, changed words). The frontend passes in
  a syntax highlighter function and draws the result.
- `xenon_finder`: the cmd-p file index and fuzzy ranking.
- `xenon_keymap`: default key bindings per platform and shortcut text. The frontend
  turns the bindings into its own accelerators.
- `xenon_ide`: the local WebSocket server that lets Claude Code and other agents treat
  Xenon as their IDE (open file, selection, diagnostics).
- `xenon_lsp`: the language server client and process host.
- `xenon_remote`: the phone remote server (pairing, workspace list, terminal streaming).
- `xenon_ssh`: SSH workspace transport and file access.
- `xenon_memory`: process memory diagnostics.

## Still GPUI-bound

- `xenon_design_system::settings` is the GPUI glue for `xenon_settings`: a global
  holding `UiSettings`, the font probe, and the clipboard actions.
- `xenon_terminal` and `xenon_editor` hold the GPUI views. The terminal state machine
  and the editor buffer, highlighting, and vim logic have not been separated from their
  views yet; that is the next candidate for extraction.
- `xenon_ui` keeps the app shell: it owns the `LiveTab` payload (GPUI entities) and
  calls the shared rules above.

## Step 2: terminal-only GTK frontend

`crates/xenon_gtk` is a daily-usable terminal Xenon: workspaces sidebar, the shared pane
tree (splits, tabs per pane), working and attention status, search, links, zoom, and the
keymap. Install it beside the GPUI app with `scripts/release/install-gtk` (binary
`~/.local/bin/xenon-gtk`, entry "Xenon (GTK)", app id `dev.xenon.Gtk`).

- **State.** Own directory `~/.xenon-gtk`, seeded from the GPUI app, never shared live
  (see `docs/project-state.md`). Cutover can drop the copy step: when GPUI is gone the
  GTK app is the only writer and can use `~/.xenon` directly.
- **Shared rules used.** `PaneContent` and the pane moves, `AttentionMap`,
  `workspace_dot`, `agent_busy_signal`, `clean_terminal_title`, `Registry::route_file`,
  `xenon_store` files, `UiSettings`, `xenon_keymap`. New in `xenon_core`:
  `TerminalActivity` (output-burst to working and settled), so the next frontend does
  not copy the rule. The GPUI terminal view still carries its own copy of that logic.
- **Since step 2.** Ctrl+P finder and Ctrl+Shift+O workspace picker (one palette shell;
  ranking and discovery from `xenon_finder`, which now also holds `discover` and
  `tree::list_dir`, moved out of `xenon_ui`), the Files section (lists every entry like
  the GPUI tree; only expanded folders are watched, at most 48), and a stand-in editor
  tab (GtkSourceView, Vim input method per settings, Ctrl+S saves; files over 4 MB or
  not UTF-8 open in the default app).
- **Git (since step 2).** The Git sidebar section (status rows, change bar, keyboard)
  and the read-only diff tab (unified or side by side, hunk navigation, open at the hunk
  line) on `xenon_git`; dirt ticks on the workspace rows. One worker thread (`git.rs`)
  owns a recursive `notify` watcher on every local workspace and runs
  `xenon_git::measure`; it sleeps between events, debounces 400ms and measures a
  workspace at most once per 1.5s, so build output and terminal bursts cost nothing and
  an idle dirty repo adds no wakeups. Path filters (`is_noise`, `is_git_meta`,
  `owner_of`) moved to `xenon_git::watch` and the GPUI app uses them too. Differences
  from the GPUI app: no draggable section height or saved fold state, GtkSourceView
  colors diff text instead of the tree-sitter highlighter, and the diff tab is not part
  of the saved layout (same as GPUI).
- **Not drawn yet.** The real editor (LSP, diff gutter,
- **Header bar and empty screen.** The header bar follows the GPUI toolbar order
  (sidebar, previous and next workspace, split right, split down, reserve pane, copy,
  paste on the left; New Terminal, Go to File, Close Workspace, Settings on the right).
  Tooltips are `Label · keys` with the keys read from the live accelerators (so the
  user's keymap shows). Split right and split down use two small symbolic icons compiled
  into a GResource (`resources/`, `build.rs`) because Adwaita has none. The empty
  workspace and the empty pane use an `AdwStatusPage` with the GPUI wording, buttons and
  shortcut list. Not brought over, because GTK has no such action yet: Go Back and Go
  Forward, Cut, Copy Clean, Toggle File Tree, the two worklist buttons, Run Task, Save
  (shown when an editor is dirty), and the Command Palette, Focus Editor and Keyboard
  Shortcuts rows' actions (the rows are listed with their keys). prose mode, Markdown),
  SSH workspaces (session kept) and the SSH part of the pickers, command palette, tab
  drag and drop, divider keyboard resize, the `xenon open` socket server. Phone remote
  gaps: tasks, mouse clicks, dim text and OSC 8 links (VTE's HTML has no markup for
  them), cursor hiding, and the phone's own light/dark choice (the phone always gets the
  Mac's terminal colors).
- **Settings.** Ctrl+, (`app.settings`) opens an `AdwPreferencesDialog` with the GPUI
  pages: Appearance (mode, theme strips, a live-preview theme picker), Fonts, Editor
  (with Markdown writing), Terminal, Agents, Language Servers, Phone Remote. Choices,
  clamps and healing come from `xenon_settings` (theme lineup, prose choices, font
  choices, skill text, `set_font_size`, `set_font_family`, `set_theme_name`); the GPUI
  settings read the same. Changes apply live and persist to the GTK state dir. The
  Markdown writing values are saved but the GTK editor does not draw prose mode yet.
- **Phone remote.** `xenon_remote` serves it; the rules both frontends share (device
  book and pairing, network and URLs, fit gate, workspace list, footer and status
  wording, keep-awake, VTE HTML to wire screen) live there, so the GPUI glue only adapts
  them. The GTK host is `remote*.rs`. Same port and paired-device file as the GPUI app
  (`remote_port`, default 17890; `~/.xenon-shared/remote_devices.json` is read in place,
  no copy), so cutover needs no re-pairing. If the other Xenon holds the port this one
  waits quietly and retries every 5 s. A terminal is mirrored (VTE `text_format(Html)`,
  coalesced to ~30/s, parsed off the UI thread) only while a phone is attached to it; an
  idle remote with a quiet attached phone costs about nothing. The phone fit pins the VTE grid to the phone's size and
  the Mac's keyboard hands it back (30 s quiet rule from `xenon_remote`).
