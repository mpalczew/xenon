# GTK migration map

A GTK 4 + VTE frontend is built beside the GPUI one in this repo. Both read and write
the same data files. GPUI is deleted at cutover. Step 0 (this list) makes the product
rules available without GPUI, so the new frontend only draws.

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
  the GPUI tree; only expanded folders are watched, at most 48), and a stand-in editor tab
  (GtkSourceView, Vim input method per settings, Ctrl+S saves; files over 4 MB or not
  UTF-8 open in the default app).
- **Not drawn yet.** The diff-status sidebar section, the real editor (LSP, diff gutter,
  prose mode, Markdown), SSH workspaces (session kept) and the SSH part of the pickers,
  command palette, settings and theme panels, tab drag and drop, divider keyboard resize,
  the `xenon open` socket server, phone remote.
- **Checks.** `scripts/xenon-gtk-verify` drives a real window over D-Bus actions and
  AT-SPI and writes screenshots; `scripts/xenon-gtk-idle` (run by `scripts/health`)
  fails on idle CPU or wakeups above budget.
