# Worklist

## Now

- [ ] GTK: Git section and diff tab
  - Rescued after the OOM as a WIP commit on `worktree-agent-a6101b1c2f9016601`.
- [ ] GTK: sidebar spacing in the Workspaces section
- [ ] GTK: the old Xenon toolbar and its key hints
  - Split down shows the wrong icon.
- [ ] GTK: the tab close button shows an icon and closes the tab
- [ ] GTK: the empty workspace screen matches GPUI word for word
- [ ] Remove macOS-only code
- [ ] GTK: Markdown writing mode
  - Rescued as a WIP commit on `worktree-agent-ad3978011b03424c7`.
- [x] GTK: Settings window (Ctrl+,)
- [x] GTK: phone remote, same pairing as the GPUI app

## Next

- [ ] GTK: wording pass, copying GPUI text word for word
  - Mac words become generic; nothing else changes.
  - Key hints use GPUI's format with Linux chords from the key map.
  - Remove the "not built yet" notes.
- [ ] GTK: drop the "(GTK)" suffix from the launcher entry and window title
- [ ] GTK: Shift+Enter sends a line feed, as the GPUI terminal did
- [ ] GTK: drag to reorder workspaces
- [ ] Cutover: the GTK app uses `~/.xenon` directly
- [ ] Cutover: rename the app to `xenon` and point `~/bin/xenon` at it
- [ ] GTK: `xenon open` socket server
- [ ] GTK: worklist panel
- [ ] GTK: command palette, tab drag and drop, divider resize from the keyboard
- [ ] GTK: SSH workspaces and the SSH part of the pickers
- [ ] Delete the GPUI frontend, its Vim engine and tree-sitter highlighting
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
