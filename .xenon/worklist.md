# Worklist

## Now

- [x] GTK: Files rows more compact; Git folds to one header line when clean
- [ ] GTK: Save button in the toolbar (shown when an editor is dirty)
- [ ] GTK: Connect Phone opens in front of Settings, not behind it
- [ ] GTK: delete the Interface font setting; the app follows the system font
- [ ] GTK: worklist view (what is still missing: waiting on the user)

## Next

- [ ] GTK: toolbar buttons still missing: Go Back, Go Forward, Cut
- [ ] GTK: empty screen rows that do nothing yet: Command Palette, Focus Editor,
      Keyboard Shortcuts
- [ ] GTK: command palette, divider resize from the keyboard
- [ ] Delete the GPUI frontend, its Vim engine and tree-sitter highlighting
  - Also delete `xenon_ide` (the Claude Code and Codex IDE servers); not ported.
  - Then delete the diff parser in `xenon_git` that only GPUI uses.
  - Also delete SSH workspaces (not ported): `xenon_ssh`, the GTK SSH page and toasts,
    `ssh://` in the picker.
- [ ] GTK: `xenon open` honours `--pane` and the column
- [ ] Phone remote gaps: tasks, mouse clicks, dim text, OSC 8 links, cursor hiding, the
      phone's own light/dark choice
- [ ] GTK: LSP, last if at all
