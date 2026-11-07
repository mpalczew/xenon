# Worklist

## Now

- [ ] GTK: Markdown writing mode
- [ ] GTK: open a file in the selected pane, or the last selected one
- [ ] GTK: Copy Clean in the toolbar
- [x] GTK: Workspaces, Files and Git sections collapse (header click, Enter, Space; Ctrl+E for Files; saved in settings)
- [ ] GTK: Settings moves and resizes like a window; pages balanced
- [ ] GTK: workspace picker gets the Browse… row its no-match message names
- [ ] GTK: Workspaces section looks bad (waiting on what, from the user)

## Next

- [ ] GTK: toolbar buttons still missing: Go Back, Go Forward, Cut, Toggle File Tree,
      worklist capture and open, Run Task, Save
- [ ] GTK: empty screen rows that do nothing yet: Command Palette, Focus Editor,
      Keyboard Shortcuts
- [ ] GTK: themes follow the system (libadwaita light and dark, GtkSourceView schemes)
  - Drop the bundled Zed themes and the theme picker.
- [ ] GTK: worklist panel
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
