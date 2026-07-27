# Keyboard-first behavior

Mouse input is additive. Every daily flow needs a complete keyboard path.
`CLAUDE.md` contains the implementation rule; this document records the current
surface and the non-obvious focus model.

## Daily keys

| Area | Keys |
|------|------|
| New terminal / file | ⌘N / ⌘⇧N |
| Open workspace / file | ⌘⇧O / ⌘P |
| Commands / help / task | ⌘⇧P / ⌘⇧/ / ⌘⇧R |
| Save / Save As | ⌘S / ⌘⇧S |
| Focus terminal / editor / Files | ⌘1 / ⌘2 / ⌘3 |
| Next focus | ⌃` |
| Workspace next / previous / close | ⌘⌥↓ / ⌘⌥↑ / ⌘⌥W |
| Tab next / previous / close | ⌃⇥ / ⌃⇧⇥ / ⌘W |
| Navigation back / forward | ⌘[ / ⌘] |
| Find next / previous | ⌘G / ⌘⇧G |
| Markdown preview | ⌘⇧V |
| LSP definition / diagnostics | F12 / F8 / ⇧F8 |

Find strips use ⌘F, Return/Shift-Return, ⌥C/W/R, and Escape. Elevated
palettes support type-to-filter, arrows, Return, and Escape. Vim mode includes
normal/visual/insert, `/` and `?`, `n`/`N`, `%`, and the supported ex commands.

## Focus ownership

GPUI actions run only on the focused node's path. The window therefore always
needs one live focus owner:

- the active terminal or editor;
- the Files tree through the shell focus handle; or
- the shell itself when no content exists.

Closing a focused surface is a focus transfer, not just removal. Overlay
dismiss and confirm paths must set the next focus target before destroying the
overlay. A dead or off-tree `FocusId` is an illegal state because app commands
then silently stop receiving actions.

## Open gaps

- Centralize teardown focus transfer across workspace, tab, pane, dirty-close,
  and overlay paths. Closing the last workspace can still leave zombie focus.
- Find replace (⌘⌥F).
- Keyboard access to terminal On-exit cycling.
- Keyboard choice between terminal path targets (editor vs default app).
- Keyboard workspace reorder.
