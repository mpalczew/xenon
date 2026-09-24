# Keyboard-first behavior

Mouse input is additive. Every daily flow needs a complete keyboard path.
`CLAUDE.md` contains the implementation rule; this document records the current
surface and the non-obvious focus model.

## Daily keys

| Area | Keys |
|------|------|
| New terminal / file | ⌘N / ⌘⇧N |
| New / open workspace | ⌘⌥N / ⌘⇧O |
| Open file | ⌘P (↩ this pane, ⌘↩ / ⌃↩ new pane to the right) |
| Commands / help / task | ⌘⇧P / ⌘⇧/ / ⌘⇧R |
| Save / Save As | ⌘S / ⌘⇧S |
| Focus terminal / editor / Files | ⌘1 / ⌘2 / ⌘3 (kinds, not pane index) |
| Next pane (leaves, then Files) | ⌃` |
| Workspace next / previous / close | ⌘⌥↓ / ⌘⌥↑ / ⌘⌥W |
| Tab next / previous / close | ⌃⇥ / ⌃⇧⇥ / ⌘W (this leaf only) |
| Navigation back / forward | ⌘[ / ⌘] |
| Find next / previous | ⌘G / ⌘⇧G |
| Copy Clean | ⌘⇧C |
| Agent skill prompt | Return install · Escape not now · ←/→ buttons |
| Markdown preview | ⌘⇧V |
| Themes | ⌘⌥T |
| LSP definition / diagnostics | F12 / F8 / ⇧F8 |

Find strips use ⌘F, Return/Shift-Return, ⌥C/W/R, and Escape. Elevated
palettes support type-to-filter, arrows, Return, and Escape. ⌘P also takes
⌘↩ / ⌃↩ (or ⌘-click) to open the file in a new pane to the right. If that
path is already open, focus the existing tab (paths stay unique). Terminal
⌘-click opens a path or URL even while a TUI has mouse reporting; Option-drag
(or hover-chrome Select text) selects and copies. Copy Clean (⌘⇧C) strips TUI
chrome from the selection, or from the clipboard if the selection is gone.
Do not bind hold-⌘ to select mode.

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

## Focus ownership

GPUI actions run on the focused node's path. An active Xenon window always has
a logical focus owner:

- the active terminal or editor;
- the Files tree through the shell focus handle; or
- the shell itself when no content exists.

The logical owner is resolved as a non-optional `FocusOwner`; with no live
surface, it is the shell. GPUI can report no focused element while the window
is inactive. In that case GPUI dispatches actions from the root node. OS-level
window blur is separate from Xenon's logical focus owner.

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
previous workspace. A dead or off-tree `FocusId` is recovered through the
logical owner; GPUI falls back to the root dispatch node when no focused
element exists.

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
