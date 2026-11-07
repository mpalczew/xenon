# Worklist

## Now

- [ ] GTK: Git section and diff tab
  - Rescued after the OOM as a WIP commit on `worktree-agent-a6101b1c2f9016601`.
- [ ] GTK: sidebar spacing in the Workspaces section
- [ ] GTK: the old Xenon toolbar and its key hints
  - Split down shows the wrong icon.
- [ ] GTK: Markdown writing mode
  - Rescued as a WIP commit on `worktree-agent-ad3978011b03424c7`.
- [x] GTK: Settings window (Ctrl+,)
- [x] GTK: phone remote, same pairing as the GPUI app

## Next

- [ ] GTK: full editor (LSP, diff gutter)
- [ ] GTK: SSH workspaces and the SSH part of the pickers
- [ ] GTK: command palette, tab drag and drop, divider resize from the keyboard
- [ ] GTK: `xenon open` socket server
- [ ] Cutover: drop GPUI and macOS, use `~/.xenon` directly

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
