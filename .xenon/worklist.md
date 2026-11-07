# Worklist

## Now

- [x] Delete the GPUI app and every crate and line of code the GTK app doesn't use
  - The GPUI crates, `third_party/`, its Vim engine and tree-sitter highlighting.
  - `xenon_ide` (Claude Code and Codex IDE servers), `xenon_lsp` and the Language
    Servers settings rows.
  - SSH workspaces: `xenon_ssh`, the SSH page and toasts, `ssh://` in the picker.
  - Code in shared crates that only GPUI used (the `xenon_git` diff parser and more).
- [x] Docs audit: delete anything redundant or out of date

## Next

- [x] Toolbar Cut button
- [x] New workspace screen: useful shortcuts are clickable rows that work
- [x] Command palette
- [x] Open HTML files in the browser (button, palette, Ctrl+Alt+O)
- [ ] `xenon open` honours `--pane` and the column
- [ ] Phone remote: cursor hiding (VTE does not expose the hide-cursor mode, so the
      phone always shows its cursor)
- [ ] Git section has no draggable height
- [ ] Editor diff gutter
- [ ] Markdown prose mode: Paper surface, quote rule, round bullets, table grid, code
      highlighting, list continuation on Return
- [ ] The diff tab is not part of the saved layout
