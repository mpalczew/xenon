# Worklist

## Now

- [x] Delete the GPUI app and every crate and line of code the GTK app doesn't use
  - The GPUI crates, `third_party/`, its Vim engine and tree-sitter highlighting.
  - `xenon_ide` (Claude Code and Codex IDE servers), `xenon_lsp` and the Language
    Servers settings rows.
  - SSH workspaces: `xenon_ssh`, the SSH page and toasts, `ssh://` in the picker.
  - Code in shared crates that only GPUI used (the `xenon_git` diff parser and more).
- [ ] Docs audit: delete anything redundant or out of date

## Next

- [ ] Toolbar buttons still missing: Go Back, Go Forward, Cut
- [ ] Empty screen rows that do nothing yet: Command Palette, Focus Editor, Keyboard
      Shortcuts
- [ ] Command palette, divider resize from the keyboard
- [ ] `xenon open` honours `--pane` and the column
- [ ] Phone remote gaps: tasks, mouse clicks, dim text, OSC 8 links, cursor hiding, the
      phone's own light/dark choice
- [ ] Language servers, written new if at all
