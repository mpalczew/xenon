# Worklist

## Now

- [x] GTK: Git section and diff tab
- [x] GTK: sidebar spacing in the Workspaces section
- [x] GTK: the old Xenon toolbar and its key hints
  - Split down has its own icon now.
  - Still missing, no GTK action yet: Go Back, Go Forward, Cut, Copy Clean, Toggle File
    Tree, worklist capture and open, Run Task, Save.
- [x] GTK: replace our tab strip with AdwTabView and AdwTabBar
  - The close button on each tab closes it.
  - No custom underline on the selected tab.
  - Drag a tab to a pane edge to split, as the GPUI app did.
- [ ] GTK: the empty workspace screen matches GPUI word for word
- [x] Remove macOS-only code
- [ ] GTK: Markdown writing mode
  - Rescued as a WIP commit on `worktree-agent-ad3978011b03424c7`.
- [x] GTK: Settings window (Ctrl+,)
- [x] GTK: phone remote, same pairing as the GPUI app

## Next

- [x] Delete every test and test script; nothing runs tests any more
  - Rust tests (about 100 files), `crates/xenon_ui/tests/`, the Python helper test.
  - `scripts/xenon-gtk-verify`, `-idle`, `-latency`, `-remote-client`,
    `scripts/xenon_cli_test.sh`, `scripts/check-test-determinism`.
  - Their steps in `bin/check`, `scripts/health` and the pre-commit hook.
- [x] GTK: A/B slots, so a new build starts beside the running one
  - `~/bin/xenon` launches the GTK app; state in `~/.xenon-a` and `~/.xenon-b`.
  - One process per slot, opens routed over each slot's socket (`xenon open`).
- [x] GTK: an open file updates when it changes on disk, as in the GPUI editor
  - Unsaved edits: show a conflict instead of reloading.
- [ ] GTK: wording pass, copying GPUI text word for word
  - Mac words become generic; nothing else changes.
  - Key hints use GPUI's format with Linux chords from the key map.
  - Remove the "not built yet" notes.
- [x] GTK: drop the "(GTK)" suffix from the launcher entry and window title
- [x] GTK: Shift+Enter sends a line feed, as the GPUI terminal did
- [x] GTK: drag to reorder workspaces
- [x] Cutover: rename the app to `xenon` and point `~/bin/xenon` at it
  - `scripts/release/install` installs the GTK app; `install-gtk` goes away.
  - `.vscode/tasks.json` keeps only the GTK launch and install tasks.
- [ ] GTK: diff tab on `similar` and two GtkSourceView panes
  - Keep side by side, unified, changed words and the jump buttons.
  - Shared scrollbar; line backgrounds from GtkSourceView marks.
- [ ] GTK: themes follow the system (libadwaita light and dark, GtkSourceView schemes)
  - Drop the bundled Zed themes and the theme picker.
- [x] GTK: `xenon open` socket server
- [ ] GTK: worklist panel
- [ ] GTK: command palette, divider resize from the keyboard
- [ ] GTK: SSH workspaces and the SSH part of the pickers
- [ ] Delete the GPUI frontend, its Vim engine and tree-sitter highlighting
  - Also delete `xenon_ide` (the Claude Code and Codex IDE servers); not ported.
- [ ] GTK: LSP, last if at all

## Done

- [x] Phone input mode in the title bar
  - Field keeps the editable line and the Codex Enter pause.
  - Direct sends each key straight to the program.
  - Remember the choice on this device.
- [x] Hide the software keyboard
- [x] Tap the terminal to click a TUI cell
  - A drag still scrolls.
  - Direct also focuses typing on that tap.
- [x] Close a terminal from the phone
  - Close in the workspace the phone is viewing.
  - The Mac moves to the next tab only when that tab was showing.
- [x] Adopt dprint for markdown and json files
  - Rust files and Cargo manifests stay on rustfmt, untouched.
  - `bin/check` gained a path-gated `dprint check` gate.
