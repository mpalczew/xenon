# Worklist

## Now

- [x] GTK: Markdown writing mode
- [x] GTK: open a file in the selected pane, or the last selected one
  - One model rule; editors and diffs now remember their pane when focused.
  - `xenon open --pane` is still ignored; every open uses the focused pane.
- [x] GTK: Copy Clean in the toolbar
- [x] GTK: Workspaces, Files and Git sections collapse (header click, Enter, Space;
      Ctrl+E for Files; saved in settings)
- [x] GTK: Settings moves and resizes like a window; pages balanced
- [x] GTK: workspace picker gets the Browse… row its no-match message names
- [x] GTK: finder, workspace and task pickers use stock libadwaita look
- [x] GTK: Workspaces section looks bad (waiting on what, from the user)

- [x] GTK: task runner (task picker, Run Build Task, inputs and trust prompts, Run
      Task button)

## Next

- [ ] GTK: toolbar buttons still missing: Go Back, Go Forward, Cut, Toggle File Tree,
      Save
- [ ] GTK: empty screen rows that do nothing yet: Command Palette, Focus Editor,
      Keyboard Shortcuts
- [x] GTK: themes follow the system, mostly by deleting code
  - Delete the Zed themes, the theme picker, the theme lineup and the mode setting.
  - Keep two small switches: terminal palette and GtkSourceView scheme, light or dark.
- [x] GTK: worklist panel and capture
  - Open Worklist opens `.xenon/worklist.md` in the Markdown writing mode; capture is a
    dialog that saves as you type. The list view's section folds and inline editing are
    not ported (edit the Markdown).
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
