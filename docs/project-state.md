# Current project decisions and gotchas

Code and focused design documents are authoritative for implementation. This file keeps
only cross-cutting decisions and traps that are expensive to rediscover.

## Current model

- A workspace is a checkout root plus a saved pane-tree session.
- The pane tree is N-ary: `Split { axis, children, sizes }`, sizes are fractions summing
  to 1 (min 0.1), no child splits on its parent's axis (the helper
  `xenon_core::flatten_split` splices and collapses). A divider moves only its two
  neighbours; it is keyed by the last leaf before it. `MAX_NEST_DEPTH` counts axis
  alternations, `MAX_SPLIT_CHILDREN` caps a row. Old saved `first/second/ratio` layouts
  load through a serde shim and are rewritten flat.
- There is no user-facing stream concept. Parallel agents use terminal tabs and/or
  multiple workspaces.
- Terminals and editors are peer surfaces in pane tab stacks. PTYs are not restored
  across restart; terminal slots reopen as fresh shells.
- The Files tree lives under Workspaces in the optional sidebar. When both are open,
  Workspaces sizes to content (or a user-pinned height); drag the split between sections
  to resize. Files takes the remainder.
- The Git section sits under Files for the active workspace (hidden for non-git
  folders). Enter on a row opens a read-only diff tab in the focused pane, reusing an
  open tab for the same path. Diff tabs are not persisted: restart drops them. They
  reload when the Git section reports a change. Auto layout is split at 860px pane width
  and up.
- Attention is per terminal tab: static warning after BEL or inferred idle-after-output;
  pulsing info while that terminal is in an agent-sized burst. The latter is local
  inference, not an agent completion event. Working outranks a settled mark on the same
  tab. The workspace row aggregates its terminals (any working → working; else any
  attention → attention). Viewing or using a tab dismisses only that tab.
- PTY session portability is the product boundary. Agent chrome uses `xenon open` (CLI +
  managed skill under `~/.agents/skills/xenon` and friends). Skill install state is
  global (`~/.xenon-skill.json`), not per-slot `settings.json`. PTYs inject
  `XENON_DATA_DIR` so open targets this instance. Dogfood (both A/B live): `which xenon`
  is still `~/bin/xenon`; A/B routing is that env plus the launcher’s `open` handler
  (`scripts/xenon`, copied to `~/bin/xenon` by `scripts/release/install`). The installed
  binary is `xenon-bin` (default `~/.local/share/xenon`). Cold start from the desktop
  entry or login: no data-dir env → last slot from `~/.xenon-active-slot` when
  `~/.xenon-a`/`~/.xenon-b` exists, else ship `~/.xenon`. Env already set is not
  overridden, so the CLI still execs the binary with slot env (0/1/2 and `xenon open` do
  not flip slots). Launching while running activates; it does not spawn the other slot.
  Public ship without the marker file is one instance at `~/.xenon`.
- No embedded web browser or webview tab. Terminal URLs and `.html` / `.htm` / `.pdf`
  hand off to the system default app. Agent computer-use stays in the harness
  (Playwright, Chrome, browser MCP).

## Load-bearing choices

- VS Code tasks.json compatibility is settled in `docs/designs/vscode-tasks.md`: each
  task is its own process in a Xenon task tab (never typed into a shell),
  extension-contributed types and `command` inputs are listed as unsupported, and
  `runOn: folderOpen` never runs without a per-workspace approval keyed to a hash of the
  tasks it covers (stored in the data dir, not the repo).
- The phone remote is tailnet-only (settled 2026-10-24): it binds the Tailscale/CGNAT
  address (100.64.0.0/10, Headscale included) and loopback, never a LAN or wildcard
  address. Plain HTTP on Wi-Fi would expose the device token and let anyone rewrite the
  page, and an open-source app cannot depend on a hosted relay, so the user's tailnet is
  the encrypted transport. Saved `remote_network` values are ignored.
- `.vscode/tasks.json` exposes only the `xenon` and `install` (`bin/install`) tasks.
  Build/fmt/lint/health run via cargo or `scripts/*`; do not invent extra task labels.
  The pre-commit hook is the gate, `scripts/health` the full run.
- `xenon open` on a file: the longest open workspace root containing it wins; otherwise
  it opens in the current workspace, even outside every root. Closed workspaces never
  reopen for a file (a closed `~` would claim all of home).
- Known open/closed workspaces outrank filesystem discovery. Workspace MRU is persisted;
  file MRU is an in-memory tiebreaker only.
- Settings share `settings.json`; partial saves use the locked load-mutate-save boundary
  and preserve fields they do not own.
- The app (`crates/xenon`, Linux) runs as A/B slots: one process per slot
  (`ApplicationFlags::NON_UNIQUE`), state in `~/.xenon-a` and `~/.xenon-b`
  (`XENON_DATA_DIR`, `XENON_SLOT`), phone-remote devices in `~/.xenon-shared`. Both
  slots use the same remote port; the second waits and retries. The app saves on change
  (sessions, registry, settings), so the launcher copies live state when it flips slots.
  Window size is written when the window closes.
- `xenon open` and `xenon <path>` reach a slot over its `ipc.sock` (`xenon_store::ipc`,
  served by `ipc.rs`). The app writes `xenon.pid` when it takes the socket. Shells keep
  `XENON_DATA_DIR`, so `xenon open` in a terminal reaches the slot that runs it. Each
  terminal also gets `XENON_WORKSPACE` (its workspace id), which `xenon open` sends so
  the file opens in the caller's own workspace, even outside its root; `--workspace
  name|path|id` overrides, and an unknown workspace is an error. `xenon open` honors
  `--pane` (`sibling` default: the next pane, or a new one to the right if there is only
  one; `focused`; `split-right`) and puts the caret at line and column.
- App id is `dev.xenon.Xenon`; the desktop entry is `xenon.desktop` with
  `StartupWMClass=dev.xenon.Xenon`.

## Gotchas

- A fresh PTY grid is tiny before first paint. Remote-only reopen must call
  `ensure_grid_size`.
- Remote output defaults to responsive delivery, without a fixed frame delay. The
  browser's Low bandwidth toggle batches all output at 30 fps, including TUI redraws,
  and persists per browser. WebSocket I/O wakes on socket readiness or queued host
  output; TCP_NODELAY avoids buffering small updates.
- a/b slots: only one instance can own the phone-remote port; the other waits
  (`StartError::PortBusy`) and takes over when it is released. Paired devices live in
  `xenon_store::shared_dir()` (`~/.xenon-shared`), not the slot dir, so a phone stays
  paired across a slot flip.
- Markdown YAML metadata requires the pulldown-cmark metadata option; otherwise the
  closing fence can become a setext heading.
- Git dirt is a right-edge tick centered on the workspace row (filled: tree differs from
  HEAD; hollow: only the index does). Hover actions occupy the same gutter and replace
  the tick while hovering. Dirt comes from `git status --porcelain=v1 -z`, not net
  numstat: a tree whose staged and unstaged changes cancel (`MM`, net zero vs HEAD) is
  IndexOnly, never clean. Status runs with `--no-optional-locks` so measuring cannot
  touch `.git/index` and re-trigger its own watcher.
- Git status is refreshed by `gio` monitors on each repository's `.git` (index, HEAD,
  refs), by window focus, and every 5 s for the active workspace while the window is
  focused. gio cannot watch a tree recursively, and one monitor per folder blew up
  memory on big trees, so edits to working-tree files are caught by that timer.
- A completed `scripts/release/install` updates the app on disk, but a running process
  keeps the old binary. Never kill Xenon to force the switch.
- Quitting Xenon kills what its terminals started. The launcher `setsid`s before exec so
  a Xenon launched from another Xenon's terminal (slot flip) outlives it; ignoring
  SIGHUP alone was not enough.
- No crash handler. A vanished instance: panics land in the slot's `xenon.log` (stderr
  goes there); a SIGSEGV or abort leaves no line. The launcher turns core dumps on, so
  `coredumpctl info PID` gives the stack.
- GTK event rule: never read or change a list model, expand a row, or mutate or destroy
  a widget inside that object's own signal handler (activate, notify, items-changed,
  gestures, drag and drop, menu actions). Capture plain values and defer with
  `glib::idle_add_local_once`. Every segfault so far (libgtk tree-list code) broke this.
- GtkSourceView's syntax tags are anonymous; code that copies them must key on the look,
  not the tag name.
- Phone page: setting `disabled` on a focused input blurs it, and iOS never gives focus
  back. `renderStale()` remembers the focused input and refocuses it on reconnect; any
  new disable-while-stale input needs the same.
- Phone text cannot carry a newline: `pty_input_bytes` turns `\n` into `\r`, which
  submits in Claude Code. A prompt newline is the `shift-enter` named key (LF via the
  terminal key map).

## Markdown prose mode

- One buffer. Prose and Source are views of the same text buffer; the parser and the
  rule for which markup shows live in `xenon_prose`, the GTK tags in `prose.rs`.
- Per-file mode and look settings persist (`xenon_store::prose`, `settings.json`).

## Open product work

- Find replace.
- Keyboard paths for terminal On-exit, open-target choice, and workspace reorder.
