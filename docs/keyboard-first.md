# Keyboard first

Everything works from the keyboard; the mouse is extra. The keyboard path is shortcuts,
not Tab: no control is a tab stop. Tooltips and menus show the shortcuts so people can
find them.

App shortcuts live in one table, `crates/xenon/src/keys.rs`. Tooltips, menus and the
command palette read their key text from it. Keys can't be changed by the user. Keys
that only work inside one view (Files, Git, diff, palettes, editor) are handled in that
view's code and listed below.

## App keys

| Action                                | Keys                                     |
| ------------------------------------- | ---------------------------------------- |
| New terminal                          | Ctrl+N                                   |
| New / open workspace                  | Ctrl+Alt+N / Ctrl+Shift+O                |
| Next / previous / close workspace     | Ctrl+Alt+Down / Ctrl+Alt+Up / Ctrl+Alt+W |
| Find a file                           | Ctrl+P                                   |
| Command palette                       | Ctrl+Shift+P                             |
| Settings                              | Ctrl+,                                   |
| Run a task / the build task           | Ctrl+Shift+R / Ctrl+Shift+B              |
| Allow / deny a folder's tasks         | Ctrl+Alt+Return / Ctrl+Alt+Backspace     |
| Next / previous tab                   | Ctrl+Tab / Ctrl+Shift+Tab                |
| Close tab / close other tabs          | Ctrl+W / Ctrl+Alt+Shift+W                |
| Next pane                             | Ctrl+`                                   |
| Split right / down                    | Ctrl+\ / Ctrl+Shift+\                    |
| Keep an empty pane                    | Ctrl+Alt+\                               |
| Focus terminal / Files / Git          | Ctrl+1 / Ctrl+3 / Ctrl+Alt+G             |
| Show or hide the sidebar              | Ctrl+Alt+B                               |
| Find / next / previous                | Ctrl+Shift+F / F3 / Shift+F3             |
| Copy / Copy Clean / paste             | Ctrl+Shift+C / Ctrl+Shift+Y / Ctrl+V     |
| Text bigger / smaller / reset         | Ctrl+= / Ctrl+- / Ctrl+0                 |
| Capture to worklist / open worklist   | Ctrl+Shift+K / Ctrl+Alt+K                |
| Undo a worklist capture (while toast) | Ctrl+Alt+Z                               |
| Open an HTML file in the browser      | Ctrl+Alt+O                               |
| Connect a phone                       | Ctrl+Shift+M                             |
| Quit                                  | Ctrl+Q                                   |

## The terminal

- In a focused terminal, Ctrl+C, X, B, E, O and S go to the shell or program, not the
  app. So do Ctrl chords the app doesn't use, like Ctrl+D, Z, R, A, K, U, L and G.
- Ctrl+V is Xenon's paste, so terminal programs don't treat it as an image paste.
- Shift+Return sends a line feed, which Claude Code and Codex read as a new line.
- Ctrl+click opens a link or file path, even while a program captures the mouse.
- Copy Clean removes terminal program decorations from the selection, or from the
  clipboard when nothing is selected.

## Views

- **Palettes** (file finder, workspace picker, commands): type to filter, Up/Down or
  Ctrl+N/Ctrl+P to move, Tab to complete a path, Return to open, Escape to close.
  Ctrl+Return opens a file in a new pane, or runs a task in a new tab. Focus goes back
  where it was.
- **Files:** Up/Down move, Right/Left open and close folders, Return opens, F2 renames,
  Delete moves to the trash, Menu or Shift+F10 opens the menu, Escape goes back to the
  terminal.
- **Git:** Ctrl+Alt+G opens and focuses the section; pressing it again folds it. Up/Down
  move, Return opens the file's diff, Escape goes back to the pane.
- **Diff tab:** n or ] next change, p or [ previous, Return, o or e open the file at
  that line, j/k scroll, Space pages, l or Ctrl+Alt+Shift+D switches the layout, q
  closes.
- **Editor:** Ctrl+S saves. Vim mode (in Settings) adds Vim keys, and `:w` saves.
- **Markdown:** Alt+1 shows it formatted, Alt+3 shows the code, and on the worklist
  Alt+4 shows the task list. A switch at the top of the tab does the same. In the
  formatted view, Return continues a list or quote and Ctrl+Return checks or unchecks a
  task.
- **Worklist capture:** Return adds a point, Alt+Up/Down picks the section, Ctrl+Return
  closes, Ctrl+Backspace deletes the item.
- **Task prompts:** Return answers, Escape cancels the run.
- **Toasts** never take focus. A toast's shortcut works from anywhere while it shows.

## Focus

Focus stays on the terminal or editor you last clicked or typed in, and commands like
close tab, split and new terminal act on that pane. When something closes, focus moves
to an editor or terminal that is still there. Palettes give focus back when they close.
Clicking a pane's frame doesn't take focus.

Missing keyboard paths are on the worklist (`.xenon/worklist.md`).
