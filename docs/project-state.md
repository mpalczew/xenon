# Current project decisions and gotchas

Code and focused design documents are authoritative for implementation. This
file keeps only cross-cutting decisions and traps that are expensive to
rediscover.

## Current model

- A workspace is a checkout root plus a saved pane-tree session.
- There is no user-facing stream concept. Parallel agents use terminal tabs
  and/or multiple workspaces.
- Terminals and editors are peer surfaces in pane tab stacks. PTYs are not
  restored across restart; terminal slots reopen as fresh shells.
- The Files tree lives under Workspaces in the optional sidebar.
- Attention is a workspace dot driven by BEL or inferred idle-after-output.
  The latter is local inference, not an agent completion event.
- The Claude IDE bridge is optional garnish. PTY session portability is the
  product boundary.

## Load-bearing choices

- Known open/closed workspaces outrank filesystem discovery. Workspace MRU is
  persisted; file MRU is an in-memory tiebreaker only.
- Cmd-P serves its cached index immediately and refreshes in the background.
  It still respects ignore rules and skips heavy directories.
- ⌘[ and ⌘] are browser-style surface/location history, not tab cycling.
- Editor and terminal find share UX but not implementation. Vim search is a
  separate modal flow with smartcase.
- The embedded Vim layer is intentionally good-enough, not a Neovim host.
- Settings geometry and chrome preferences share `settings.json`; partial
  saves must preserve fields they do not own.

## Gotchas

- GPUI typed text arrives through `EntityInputHandler`, not ordinary key-down
  handlers. Printable key handlers must return unhandled.
- Palette destruction without a live restore target creates keyboard void.
  Closing the last workspace has the same zombie-focus risk.
- A fresh PTY grid is tiny before first paint. Remote-only reopen must call
  `ensure_grid_size`.
- Markdown YAML metadata requires the pulldown-cmark metadata option; otherwise
  the closing fence can become a setext heading.
- Markdown emphasis highlighting depends on `injection.include-children` and
  styled text runs, not just a grammar and colors.
- Overlay scrollbars require caret-follow calculations to reserve the bar.
- Git dirt stays flush-right; hover actions must occupy the same gutter.
- A completed `project install` updates the app on disk, but a running process
  keeps the old binary. Never kill Xenon to force the switch.

## Open product work

- Make focus ownership unrepresentable as an invalid state.
- Find replace.
- Keyboard paths for terminal On-exit, open-target choice, and workspace
  reorder.
- Finish the GitHub rename and public packaging in `docs/release.md`.
