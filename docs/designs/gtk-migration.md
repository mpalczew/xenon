# GTK migration map

A GTK 4 + VTE frontend is built beside the GPUI one in this repo. Both read and
write the same data files. GPUI is deleted at cutover. Step 0 (this list) makes
the product rules available without GPUI, so the new frontend only draws.

## Shared model crates

None of these depend on GPUI. A frontend uses them for the following.

- `xenon_core`: the data model and the rules on it. Workspaces and the
  registry, including which open workspace a CLI or agent file belongs to
  (`Registry::route_file`). The saved pane layout and the live pane tree
  (`PaneTree`, `PaneContent`: split, move tab, close tab, unsplit on last tab,
  parked empty panes, depth limits), generic over the frontend's own tab type.
  Terminal attention and working status (`AttentionMap`, `workspace_dot`, the
  agent-detection thresholds and `IDLE_AFTER`). Terminal title cleanup and tab
  labels. VS Code task parsing.
- `xenon_store`: reads and writes everything under `~/.xenon` (registry,
  sessions, `settings.json`), the CLI argument parser, and the local socket
  protocol that `xenon open` uses (`OpenFileSpec`, `OpenPane`).
- `xenon_settings`: app-wide settings as plain values (`UiSettings`): font
  faces and sizes with their clamps, wrap defaults, prose look, theme choice.
  It also holds the bundled font bytes and font-family aliasing. A frontend
  keeps one `UiSettings` in its own state, implements `FontProbe` over its text
  system, and calls `persist` after a change.
- `xenon_git`: git status for the sidebar (branch, changed files, line counts),
  and the read-only diff model (hunks, side-by-side rows, changed words). The
  frontend passes in a syntax highlighter function and draws the result.
- `xenon_finder`: the cmd-p file index and fuzzy ranking.
- `xenon_keymap`: default key bindings per platform and shortcut text. The
  frontend turns the bindings into its own accelerators.
- `xenon_ide`: the local WebSocket server that lets Claude Code and other
  agents treat Xenon as their IDE (open file, selection, diagnostics).
- `xenon_lsp`: the language server client and process host.
- `xenon_remote`: the phone remote server (pairing, workspace list, terminal
  streaming).
- `xenon_ssh`: SSH workspace transport and file access.
- `xenon_memory`: process memory diagnostics.

## Still GPUI-bound

- `xenon_design_system::settings` is the GPUI glue for `xenon_settings`: a
  global holding `UiSettings`, the font probe, and the clipboard actions.
- `xenon_terminal` and `xenon_editor` hold the GPUI views. The terminal state
  machine and the editor buffer, highlighting, and vim logic have not been
  separated from their views yet; that is the next candidate for extraction.
- `xenon_ui` keeps the app shell: it owns the `LiveTab` payload (GPUI entities)
  and calls the shared rules above.
