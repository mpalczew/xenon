# Keyboard-first behavior

Mouse input is additive. Every daily flow needs a complete keyboard path. `AGENTS.md`
contains the implementation rule; this document records the current surface and the
non-obvious focus model.

The keyboard path is shortcuts. There is no tab order: no control is a tab stop.
Tooltips and menus exist to make the shortcuts discoverable.

## Daily keys

Tables write the main modifier as ⌘, which is Ctrl on Linux (Ctrl+P, Ctrl+Shift+P, ...),
with the exceptions listed under "Linux differences" below. Not every row is built in
the GTK app yet; `.xenon/worklist.md` lists what is missing.

| Area                                                                                                                                           | Keys                                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| New terminal / file                                                                                                                            | ⌘N / ⌘⇧N                                                   |
| New / open workspace                                                                                                                           | ⌘⌥N / ⌘⇧O                                                  |
| Open file                                                                                                                                      | ⌘P (↩ this pane, ⌘↩ / ⌃↩ new pane to the right)            |
| Commands / help / task                                                                                                                         | ⌘⇧P / ⌘⇧/ / ⌘⇧R                                            |
| Run build task (default build task, else the only one, else the picker)                                                                        | ⌘⇧B                                                        |
| Task picker: run per its presentation · run in a new tab · close                                                                               | ↩ · ⌘↩ · Escape                                            |
| Task input prompt (`inputs`): answer · cancel the run · choose (pick lists)                                                                    | ↩ · Escape · ↑/↓                                           |
| folderOpen consent card (non-modal, shown until answered): allow · don't allow                                                                 | ⌘⌥↩ · ⌘⌥⌫                                                  |
| Save / Save As                                                                                                                                 | ⌘S / ⌘⇧S                                                   |
| Capture task / open worklist                                                                                                                   | ⌘⇧K / ⌘⌥K                                                  |
| Toast: undo capture / dismiss                                                                                                                  | ⌘Z / ⌘.                                                    |
| Focus terminal / editor / Files                                                                                                                | ⌘1 / ⌘2 / ⌘3 (kinds, not pane index)                       |
| Focus Git section (again: fold it, back to the pane)                                                                                           | ⌘⌥G                                                        |
| Git section: move · open diff · back to the pane                                                                                               | ↑/↓ · ↩ · Escape                                           |
| Diff tab: cycle Auto / Split / Unified · scroll                                                                                                | ⌘⌥⇧D · ↑/↓, Space, PageUp/PageDown, Home/End               |
| Next pane (leaves, then Files)                                                                                                                 | ⌃`                                                         |
| Workspace next / previous / close                                                                                                              | ⌘⌥↓ / ⌘⌥↑ / ⌘⌥W                                            |
| Tab next / previous / close                                                                                                                    | ⌃⇥ / ⌃⇧⇥ / ⌘W (this leaf only)                             |
| Find next / previous                                                                                                                           | ⌘G / ⌘⇧G                                                   |
| Copy Clean                                                                                                                                     | ⌘⇧C                                                        |
| Agent skill prompt                                                                                                                             | Return install · Escape not now · ←/→ buttons              |
| Markdown mode: Prose / Split / Source (Split again swaps the active pane)                                                                      | ⌥1 / ⌥2 / ⌥3                                               |
| Markdown: toggle Prose and Source                                                                                                              | ⌘⇧V                                                        |
| Markdown: Paper · flip task box                                                                                                                | ⌥P · ⌘Return                                               |
| Markdown prose: body size bigger · smaller · reset                                                                                             | ⌘= · ⌘− · ⌘0                                               |
| Markdown prose: Enter · soft break · outdent/indent item                                                                                       | Return (continues lists and quotes) · ⇧Return · ⇧Tab / Tab |
| Markdown prose with vim on: vim acts on the source; Insert keeps the prose keys above; bare `j`/`k` move one display row; Escape leaves Insert | vim keys                                                   |
| Connect Phone sheet · copy link · keep awake while connected · close                                                                           | ⌘⇧M · ⌘C · ⌘K · Escape / Return                            |
| Markdown: follow link or `path` code span under cursor (source or prose; ⌘-click)                                                              | `gd` / ⌘-click                                             |

Toasts never take focus or Escape. A toast's action shows its shortcut and that shortcut
runs it from anywhere while the toast is visible. ⌘Z undoes a worklist capture while its
toast is showing; otherwise ⌘Z passes through to the focused surface. ⌘. dismisses the
toast (and passes through when none shows).

Find strips use ⌘F, Return/Shift-Return, ⌥C/W/R, and Escape. Elevated palettes support
type-to-filter, arrows, Return, and Escape. ⌘P also takes ⌘↩ / ⌃↩ (or ⌘-click) to open
the file in a new pane to the right. If that path is already open, focus the existing
tab (paths stay unique). Terminal ⌘-click opens a path or URL even while a TUI has mouse
reporting; Option-drag (or hover-chrome Select text) selects and copies. Copy Clean
(⌘⇧C) strips TUI chrome from the selection, or from the clipboard if the selection is
gone. Do not bind hold-⌘ to select mode.

Worklist capture focuses its title on every open and saves as you type. Return adds a
point, Escape or ⌘Return closes, and ⌘⌫ deletes the item. When the file has sections, an
"Into ‹ Section ›" chip sits under the editor and ⌥↑ / ⌥↓ (or the ‹ › buttons) choose
the section until the first save. The worklist opens in a closable tab. In List view,
↑/↓ walk section headers and items together, Space completes an item, Return edits it,
and Add opens an inline field. On a header: ←/→ fold and unfold, Space toggles, Return
adds an item to that section, ⌘Return renames it (Return commits, Escape cancels; a
blank name deletes an empty section), and ⇧Return starts a new section after the current
one. Clicking a header folds it. There is no key or button to move items between
sections; that is a Markdown edit. On an empty list, Return adds the first item. Item
editing saves as you type. Escape closes it. ⌘⌫ deletes the item. An empty virtual
worklist can be saved with ⌘S. In the GTK app the Worklist button and ⌥4 show the list,
and the Markdown button, ⌥1 and ⌥3 show the file as text; the choice is remembered per
file.

The sidebar Workspaces `+` opens a two-item menu (⌘⌥N New workspace, ⌘⇧O Open
workspace). Arrow keys and Return choose a row. New workspace accepts the directory name
first, then a parent folder. A typed path (`~/src`) lists that directory and its
subfolders, shallower first. Bare text still fuzzy-filters folders under `~`. Enter
creates using the named folder under the selected parent; Tab does not complete paths.
While that field is focused, ⌘V pastes into the name, not the tab behind it.

Tabs and panes stay separate verbs. ⌃⇥ cycles tabs in the focused leaf. The overflow
count (``N ▾``) lists every tab in that leaf; arrows, Return, Delete, and Escape operate
the list while it is open. ⌃` cycles leaves, then Files. Do not make ⌃⇥ wrap across
panes (VS Code / Zed / IntelliJ use ⌃⇥ as an MRU picker, not sequential wrap). ⌘1/2/3
stay last-terminal / last-editor / Files.

Vim mode includes normal/visual/visual-line/visual-block (`Ctrl-v`), `/` and `?`,
`n`/`N`, `%`, join/indent/number-bump/scroll-center edits, and the supported ex
commands.

## Customizing keys

Shortcuts are bindings you can change. Create `~/.xenon/keymap.json` (`XENON_DATA_DIR`
moves it). The file is a JSON array of `{context?, bindings}` sections (comments and
trailing commas are fine); your entries layer on top of the defaults, and a later entry
wins:

```json
[
  {
    "bindings": {
      "ctrl-alt-p": "xenon::FilePalette",
      "cmd-shift-p": null
    }
  },
  {
    "context": "Terminal",
    "bindings": { "ctrl-b": "xenon::ToggleSidebar" }
  }
]
```

- Keys are `ctrl`, `alt`, `shift`, `cmd` (Super on Linux) plus a key, and a space
  separates a chord sequence (`ctrl-k ctrl-t`).
- `null` unbinds a default.
- Action names are `xenon::` plus the command, for example `xenon::NewTerminal`; the
  defaults in `crates/xenon_keymap/src/defaults.rs` list them.
- `context` is optional. The GTK app reads global entries and `Terminal` entries. On the
  focused terminal a global binding and a `Terminal` binding tie; the later entry wins.
- The file is read at startup. A missing file means defaults only; Xenon never creates
  it on its own.
- Tooltips, menus, and the shortcut list show your bindings.

### Linux differences

Linux uses Ctrl where the tables say ⌘, plus:

| Command                         | Linux                                            |
| ------------------------------- | ------------------------------------------------ |
| Undo capture (toast)            | Ctrl+Alt+Z (Ctrl+Z is the shell's suspend)       |
| Copy Clean                      | Ctrl+Shift+Y (Ctrl+Shift+C copies in a terminal) |
| Terminal find / next / previous | Ctrl+Shift+F / F3 / Shift+F3                     |
| Terminal copy / paste           | Ctrl+Shift+C / Ctrl+V (or Ctrl+Shift+V)          |
| Quit                            | Ctrl+Q                                           |

In the focused terminal, Ctrl+C, X, B, E, O, and S reach the shell or TUI instead of the
app shortcut (Ctrl+C is SIGINT). Ctrl+V is Xenon paste, which prevents terminal TUIs
from treating it as image paste. Other Ctrl chords that are not listed here, such as
Ctrl+D, Z, R, A, K, U, L, and Ctrl+G, were never app shortcuts and go to the PTY. App
shortcuts win over the shell for Ctrl+P, N, W, J, `\`, and the rest of the tables.
Editor word and file motion on Linux is Ctrl+←/→ and Ctrl+Home/End; with vim mode on,
Ctrl+A, Z, C stay with vim.

### Built in the GTK app

`crates/xenon` builds its accelerators from the same Linux defaults and
`keymap.json` in the slot dir (the layered file works as above), for the commands it
has: new terminal, new and open workspace, tab next, previous and close, close other
tabs, next pane, workspace next, previous and close, split right and down, reserve empty
pane, focus terminal (Ctrl+1), focus the Files tree (Ctrl+3), file finder (Ctrl+P),
workspace picker (Ctrl+Shift+O; Ctrl+O too when focus is not in a terminal), terminal
find, copy, paste, font size, and quit. Keys the shell owns stay with the shell.
Differences: Ctrl+Alt+B toggles the sidebar (Ctrl+B is the shell's). In the Files tree
↑/↓ move, →/← open and close a folder (or step in and out), Return opens a file, and
Escape returns to the terminal. The finder and the picker are one palette: type to
filter, ↑/↓ or Ctrl+N/Ctrl+P move, Return opens, Tab completes a path (picker), Escape
closes, and focus goes back to where it was. Git: Ctrl+Alt+G focuses the Git section
(opening it); pressed again it folds the section and returns to the pane. ↑/↓ move,
Return opens the file's diff tab, Escape returns to the pane. In a diff tab: n or ] next
hunk, p or [ previous hunk, Return, o or e open the file in an editor tab at the hunk's
line, j/k and ↑/↓ scroll, Space and PageUp/PageDown page, l or Ctrl+Alt+Shift+D cycles
Auto / Split / Unified, q closes the tab (Ctrl+W too). Button and menu hints show these
chords from the same key map. While a palette is open the other app shortcuts wait.
Focus follows the terminal that has the cursor, so a click or a key in a pane makes it
the pane that commands target.

## Focus ownership

Focus follows the terminal or editor the user last clicked or typed in, and that pane is
the one commands target (Ctrl+W, Ctrl+Tab, split, new terminal). Closing a focused
surface, dismissing a palette, or switching workspace moves focus to the remaining
editor or terminal of the target workspace, never to a destroyed overlay. Palettes
return focus to the widget that had it. Clicking pane chrome must not steal focus.

## Open gaps

- Find replace (⌘⌥F).
- Keyboard access to terminal On-exit cycling.
- Keyboard choice between terminal path targets (editor vs default app).
- Keyboard workspace reorder.
- Make ⌃` (next pane) easier to discover in help / empty chrome.
