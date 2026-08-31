# Keyboard-first behavior

Mouse input is additive. Every daily flow needs a complete keyboard path.
`CLAUDE.md` contains the implementation rule; this document records the current
surface and the non-obvious focus model.

## Daily keys

| Area | Keys |
|------|------|
| New terminal / file | ⌘N / ⌘⇧N |
| Open workspace / file | ⌘⇧O / ⌘P (↩ this pane, ⌘↩ / ⌃↩ new pane to the right) |
| Commands / help / task | ⌘⇧P / ⌘⇧/ / ⌘⇧R |
| Save / Save As | ⌘S / ⌘⇧S |
| Focus terminal / editor / Files | ⌘1 / ⌘2 / ⌘3 (kinds, not pane index) |
| Next pane (leaves, then Files) | ⌃` |
| Workspace next / previous / close | ⌘⌥↓ / ⌘⌥↑ / ⌘⌥W |
| Tab next / previous / close | ⌃⇥ / ⌃⇧⇥ / ⌘W (this leaf only) |
| Navigation back / forward | ⌘[ / ⌘] |
| Find next / previous | ⌘G / ⌘⇧G |
| Markdown preview | ⌘⇧V |
| LSP definition / diagnostics | F12 / F8 / ⇧F8 |

Find strips use ⌘F, Return/Shift-Return, ⌥C/W/R, and Escape. Elevated
palettes support type-to-filter, arrows, Return, and Escape. ⌘P also takes
⌘↩ / ⌃↩ (or ⌘-click) to open the file in a new pane to the right. If that
path is already open, focus the existing tab (paths stay unique).

Tabs and panes stay separate verbs. ⌃⇥ cycles tabs in the focused leaf.
⌃` cycles leaves, then Files. Do not make ⌃⇥ wrap across panes (VS Code /
Zed / IntelliJ use ⌃⇥ as an MRU picker, not sequential wrap). ⌘1/2/3 stay
last-terminal / last-editor / Files.

Vim mode includes normal/visual/visual-line/visual-block (`Ctrl-v`), `/` and
`?`, `n`/`N`, `%`, join/indent/number-bump/scroll-center edits, and the
supported ex commands.

## Focus ownership

GPUI actions run only on the focused node's path. The window therefore always
needs one live focus owner:

- the active terminal or editor;
- the Files tree through the shell focus handle; or
- the shell itself when no content exists.

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
overlay. A dead or off-tree `FocusId` is an illegal state because app commands
then silently stop receiving actions.

Teardown routes through one focus-transfer helper. It targets the remaining
editor/terminal when available and otherwise focuses the shell, including
asynchronous dirty-close paths without a `Window` handle (PTY auto-close after
Ctrl-D queues the remaining leaf for the next paint). Clicking pane chrome
must `prevent_default` so the shell `track_focus` handle does not steal.

## Open gaps

- Find replace (⌘⌥F).
- Keyboard access to terminal On-exit cycling.
- Keyboard choice between terminal path targets (editor vs default app).
- Keyboard workspace reorder.
- Make ⌃` (next pane) easier to discover in help / empty chrome.
