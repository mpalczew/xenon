# Keyboard-first behavior

Mouse input is additive. Every daily flow needs a complete keyboard path.
`AGENTS.md` contains the implementation rule; this document records the current
surface and the non-obvious focus model.

The keyboard path is shortcuts. There is no tab order: no control is a tab
stop. Tooltips and menus exist to make the shortcuts discoverable.

## Daily keys

Tables show macOS keys. On Linux ⌘ is Ctrl (Ctrl+P, Ctrl+Shift+P, ...) with the
exceptions listed under "Linux differences" below.

| Area | Keys |
|------|------|
| New terminal / file | ⌘N / ⌘⇧N |
| New / open workspace | ⌘⌥N / ⌘⇧O |
| Open workspace picker: `ss` → `ssh://`, complete a host or remote folder (never opens) | ⇥ (↩ also completes a host row and opens a folder row; Esc closes) |
| Open file | ⌘P (↩ this pane, ⌘↩ / ⌃↩ new pane to the right) |
| Commands / help / task | ⌘⇧P / ⌘⇧/ / ⌘⇧R |
| Save / Save As | ⌘S / ⌘⇧S |
| Capture task / open worklist | ⌘⇧K / ⌘⌥K |
| Toast: undo capture / dismiss | ⌘Z / ⌘. |
| Focus terminal / editor / Files | ⌘1 / ⌘2 / ⌘3 (kinds, not pane index) |
| Next pane (leaves, then Files) | ⌃` |
| Workspace next / previous / close | ⌘⌥↓ / ⌘⌥↑ / ⌘⌥W |
| Tab next / previous / close | ⌃⇥ / ⌃⇧⇥ / ⌘W (this leaf only) |
| Navigation back / forward | ⌘[ / ⌘] |
| Find next / previous | ⌘G / ⌘⇧G |
| Copy Clean | ⌘⇧C |
| Agent skill prompt | Return install · Escape not now · ←/→ buttons |
| Markdown preview / previous · next heading | ⌘⇧V / [ · ] |
| Wrap lines | ⌥Z |
| Themes panel · browse · keep · revert | ⌘⌥T · ↑/↓ family, ←/→ dark/light (live) · Return · Escape |
| Settings: page · row · value · toggle | ⌘1–⌘7 · ↑/↓ · ←/→ (segments, sizes, theme swatches) · Space/Return |
| Settings: search · back out | ⌘F or just type · Escape (dropdown, then search, then window) |
| Connect Phone sheet · copy link · keep awake while connected · close | ⌘⇧M · ⌘C · ⌘K · Escape / Return |
| LSP definition / diagnostics | F12 / F8 / ⇧F8 |

Toasts never take focus or Escape. A toast's action shows its shortcut and
that shortcut runs it from anywhere while the toast is visible. ⌘Z undoes a
worklist capture while its toast is showing; otherwise ⌘Z passes through to the
focused surface. ⌘. dismisses the toast (and passes through when none shows).

Find strips use ⌘F, Return/Shift-Return, ⌥C/W/R, and Escape. Elevated
palettes support type-to-filter, arrows, Return, and Escape. ⌘P also takes
⌘↩ / ⌃↩ (or ⌘-click) to open the file in a new pane to the right. If that
path is already open, focus the existing tab (paths stay unique). Terminal
⌘-click opens a path or URL even while a TUI has mouse reporting; Option-drag
(or hover-chrome Select text) selects and copies. Copy Clean (⌘⇧C) strips TUI
chrome from the selection, or from the clipboard if the selection is gone.
Do not bind hold-⌘ to select mode.

Worklist capture focuses its title on every open and saves as you type.
Return adds a point, Escape or ⌘Return closes, and ⌘⌫ deletes the item. When
the file has sections, an "Into ‹ Section ›" chip sits under the editor and
⌥↑ / ⌥↓ (or the ‹ › buttons) choose the section until the first save. The worklist opens in a closable
tab. In List view, ↑/↓ walk section headers and items together, Space completes an
item, Return edits it, and Add opens an inline field. On a header: ←/→ fold and
unfold, Space toggles, Return adds an item to that section, ⌘Return renames it
(Return commits, Escape cancels; a blank name deletes an empty section), and ⇧Return
starts a new section after the current one. Clicking a header folds it. There is no
key or button to move items between sections; that is a Markdown edit. On an empty list, Return adds the first item. Item editing saves as you type. Escape closes it. ⌘⌫ deletes
the item. An empty virtual worklist can be saved with ⌘S.

The sidebar Workspaces `+` opens a two-item menu (⌘⌥N New workspace, ⌘⇧O Open
workspace). Arrow keys and Return choose a row. New workspace accepts the
directory name first, then a parent folder. A typed path (`~/src`) lists that
directory and its subfolders, shallower first. Bare text still fuzzy-filters
folders under `~`. Enter creates using the named folder under the selected
parent; Tab does not complete paths.
While that field is focused, ⌘V pastes into the name, not the tab behind it.

Tabs and panes stay separate verbs. ⌃⇥ cycles tabs in the focused leaf.
The overflow count (`N ▾`) lists every tab in that leaf; arrows, Return, Delete, and Escape operate the list while it is open.
⌃` cycles leaves, then Files. Do not make ⌃⇥ wrap across panes (VS Code /
Zed / IntelliJ use ⌃⇥ as an MRU picker, not sequential wrap). ⌘1/2/3 stay
last-terminal / last-editor / Files.

Vim mode includes normal/visual/visual-line/visual-block (`Ctrl-v`), `/` and
`?`, `n`/`N`, `%`, join/indent/number-bump/scroll-center edits, and the
supported ex commands.

## Customizing keys

Shortcuts are bindings you can change. Run **Open Keymap** from the command
palette (⌘⇧P) to create and open `~/.xenon/keymap.json` (`XENON_DATA_DIR`
moves it). The file is a JSON array in zed's keymap shape (comments and
trailing commas are fine); your entries layer on top of the defaults, and a
later entry wins:

```json
[
  {
    "bindings": {
      "ctrl-alt-p": "xenon::FilePalette",
      "cmd-shift-n": null
    }
  },
  {
    "context": "Terminal",
    "bindings": { "ctrl-b": "xenon::ToggleSidebar" }
  }
]
```

- Keys are `ctrl`, `alt`, `shift`, `cmd` (Super on Linux) plus a key, and a
  space separates a chord sequence (`ctrl-k ctrl-t`).
- `null` unbinds a default.
- Action names are `xenon::` plus the command, for example `xenon::NewTerminal`
  (see `actions!` in `crates/xenon_ui/src/lib.rs`), `xenon_editor::Find`,
  `xenon_terminal::Find`, `xenon_clipboard::Copy`.
- `context` is a gpui context predicate. Useful ones: `Editor`, `EditorFind`,
  `Terminal`, `TerminalFind`. Omit it for a global binding. On the focused
  terminal a global binding and a `Terminal` binding tie; the later entry wins.
- Saving the file reloads it. A mistake (bad JSON, unknown action, bad
  keystroke) shows a notice and keeps the previous keys. A missing file means
  defaults only; Xenon never creates it on its own.
- Tooltips, menus, and the shortcut list show your bindings.

### Linux differences

Linux uses Ctrl where the tables say ⌘, plus:

| Command | macOS | Linux |
|---------|-------|-------|
| Undo capture (toast) | ⌘Z | Ctrl+Alt+Z (Ctrl+Z is the shell's suspend) |
| Copy Clean | ⌘⇧C | Ctrl+Shift+Y (Ctrl+Shift+C copies in a terminal) |
| Themes | ⌘⌥T | Ctrl+Alt+Shift+T (Ctrl+Alt+T opens a terminal on Ubuntu) |
| Navigation back / forward | ⌘[ / ⌘] | Ctrl+Alt+- / Ctrl+Alt+_ (Ctrl+[ is Escape) |
| Terminal find / next / previous | ⌘F / ⌘G / ⌘⇧G | Ctrl+Shift+F / F3 / Shift+F3 |
| Terminal copy / paste | ⌘C / ⌘V | Ctrl+Shift+C / Ctrl+Shift+V |
| Quit | ⌘Q | Ctrl+Q |

In the focused terminal, Ctrl+C, X, V, B, E, O, and S reach the shell or TUI
instead of the app shortcut (Ctrl+C is SIGINT; Ctrl+B, E, O follow zed). Other
Ctrl chords that are not listed here, such as Ctrl+D, Z, R, A, K, U, L, and
Ctrl+G, were never app shortcuts and go to the PTY. App shortcuts win over the
shell for Ctrl+P, N, W, J, `\`, and the rest of the tables. Editor word and file
motion on Linux is Ctrl+←/→ and Ctrl+Home/End; with vim mode on, Ctrl+A, Z, C
stay with vim.

## Focus ownership

GPUI actions run on the focused node's path. An active Xenon window always has
a logical focus owner:

- the active terminal or editor;
- the Files tree through the shell focus handle; or
- the shell itself when no content exists.

The logical owner is resolved as a non-optional `FocusOwner`; with no live
surface, it is the shell. OS-level window blur is separate from Xenon's
logical focus owner.

Unreachable focus is not a safe state. GPUI dispatches from the focused
element's node in the last rendered frame; when the focused handle is dropped
or alive but unrendered, it falls back to the root dispatch node, the view
wrapper *above* the `XenonApp` key context, so every app binding misses.
`app/focus_guard.rs` enforces "the app root contains focus": on GPUI's
focus-lost signal, and as a last resort on any keystroke that finds focus
unreachable (then that key is replayed). Repair targets the active
workspace's leaf, else the shell. Explicit transfers below are still preferred
(they pick the right target); the guard makes a forgotten one recoverable.

Two layers must not disagree:

- **GPUI** `FocusHandle` is who actually receives keys (and the purple pane
  ring). Clicking or typing in a surface moves this.
- **Session** `LiveContent.focused` is which leaf ⌘W / ⌃⇥ / split / ⌘N target.
  Tab-chip clicks already updated both. Body clicks used to move only GPUI, so
  a split could show a tab underline, no pane ring, and ⌘W closed the other
  half. Commands now follow GPUI when a surface owns it; clicks/typing also
  write the session leaf.

The pane ring is GPUI-only. The tab underline is “this tab is active in its
leaf,” which is why a split can underline two tabs at once.

Closing a focused surface is a focus transfer, not just removal. Overlay
dismiss and confirm paths must set the next focus target before destroying the
overlay. Switching workspace (⌘⇧O, sidebar, ⌘⌥↓/↑, command palette) is the
same transfer: land on that workspace's focused leaf, not the overlay or the
previous workspace. A dead or off-tree `FocusId` is recovered by the
focus-lost guard above.

Teardown routes through one focus-transfer helper. It targets the remaining
editor/terminal when available and otherwise focuses the shell, including
asynchronous dirty-close paths without a `Window` handle (PTY auto-close after
Ctrl-D queues the remaining leaf for the next paint). Clicking pane chrome
must `prevent_default` so the shell `track_focus` handle does not steal.

Shortcut and focus-transfer diagnostics are written to `xenon.log` at info
level. For ⌘N / ⌘⇧O they record the raw key event and GPUI/Xenon focus owners,
then whether the action handler ran. Last-tab close records the focus transfer.

## Open gaps

- Find replace (⌘⌥F).
- Keyboard access to terminal On-exit cycling.
- Keyboard choice between terminal path targets (editor vs default app).
- Keyboard workspace reorder.
- Make ⌃` (next pane) easier to discover in help / empty chrome.
