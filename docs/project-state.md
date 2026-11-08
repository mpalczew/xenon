# Project state

Cross-cutting decisions and traps for working on Xenon. The code is the final word on
details. For the product, see `PRODUCT.md`. For keys and focus, see
`docs/keyboard-first.md`. For the phone remote, see `docs/designs/mobile-remote-v2.md`.
For VS Code tasks, see `docs/designs/vscode-tasks.md`.

## Layout and state

- The pane tree is N-ary: `Split { axis, children, sizes }`. Sizes are fractions that
  add up to 1, with a minimum of 0.1. No child splits on its parent's axis;
  `xenon_core::flatten_split` merges those. A divider moves only its two neighbours.
  `MAX_NEST_DEPTH` counts axis changes, and `MAX_SPLIT_CHILDREN` caps a row.
- Diff tabs are not saved. They are gone after a restart.
- Settings share one `settings.json`. A partial save loads, changes and saves under a
  lock, and keeps the fields it does not own.
- Window size is written when the window closes. Everything else saves on change.

## Slots and the launcher

- Xenon runs as two slots, A and B, one process each (`ApplicationFlags::NON_UNIQUE`).
  State lives in `~/.xenon-a` and `~/.xenon-b`. A copy installed without those
  directories is one instance at `~/.xenon`.
- When the launcher flips slots it copies the live state across. It also `setsid`s
  before it execs, so a Xenon started from another Xenon's terminal outlives it.
  Ignoring SIGHUP alone was not enough.
- The launcher is `scripts/xenon`, and `scripts/release/install` copies it to
  `~/bin/xenon`. The app binary is `xenon-bin` in `~/.local/share/xenon`. The last slot
  used is in `~/.xenon-active-slot`. If `XENON_DATA_DIR` is already set, the launcher
  leaves it alone.
- Starting Xenon while it runs activates it. It does not start the other slot.
- The app id is `dev.xenon.Xenon`. The desktop entry is `dev.xenon.Xenon.desktop`, with
  `StartupWMClass=dev.xenon.Xenon`.
- A finished `scripts/release/install` updates the files on disk, but a running process
  keeps the old binary. Never kill Xenon to switch.
- Quitting Xenon kills whatever its terminals started.

## `xenon open`

- `xenon open` and `xenon <path>` reach a slot through its `ipc.sock`
  (`xenon_store::ipc`, served by `crates/xenon/src/ipc.rs`). The app writes `xenon.pid`
  when it takes the socket. The Xenon skill for agents installs globally, and its state
  is in `~/.xenon-skill.json`.
- Each terminal gets `XENON_DATA_DIR` and `XENON_WORKSPACE`. So `xenon open` in a
  terminal reaches the slot that runs it and opens the file in that terminal's own
  workspace, even outside the workspace's folder. `--workspace name|path|id` overrides
  it. An unknown workspace is an error.
- `--pane` picks where the file goes. `sibling` is the default: the next pane, or a new
  one on the right if there is only one. The others are `focused` and `split-right`. The
  caret goes to the line and column you give.
- Without `XENON_WORKSPACE`, the longest open workspace folder that contains the file
  wins. If none does, it opens in the current workspace. A closed workspace never
  reopens for a file, because a closed `~` would claim all of home.
- Open and closed workspaces we already know about rank above anything found by scanning
  the disk. The workspace MRU is saved. The file MRU is only a tiebreaker in memory.

## Git

- Dirt comes from `git status --porcelain=v1 -z`, not from a net line count. A tree
  whose staged and unstaged changes cancel out still counts as dirty in the index, never
  clean.
- Git runs with `--no-optional-locks`, so measuring cannot touch `.git/index` and
  trigger its own watcher.
- Status refreshes on `gio` monitors of each repository's `.git`, on window focus, and
  every 5 seconds for the active workspace while the window has focus. gio cannot watch
  a tree recursively, and one monitor per folder used too much memory on big trees. So
  the timer catches edits to working files.

## Phone remote

- It is tailnet-only. It binds the Tailscale address (100.64.0.0/10, so Headscale too)
  and loopback, never a LAN or wildcard address. Plain HTTP on Wi-Fi would expose the
  device token and let anyone rewrite the page. An open-source app cannot rely on a
  hosted relay, so the user's tailnet is the encrypted transport.
- Only one slot can hold the remote port. The other waits (`StartError::PortBusy`) and
  takes over when it frees up. Paired devices are in `~/.xenon-shared`, not in a slot
  folder, so a phone stays paired when slots flip.
- Output is sent right away by default. The page's Low bandwidth toggle batches output
  instead, and the browser remembers the choice. Sockets set `TCP_NODELAY`.
- Setting `disabled` on a focused input blurs it, and iOS never gives focus back.
  `renderStale()` in the page remembers the focused input and refocuses it on reconnect.
  Any new input that gets disabled while stale needs the same.
- Phone text cannot contain a newline. `pty_text_bytes` turns `\n` into `\r`, which
  submits in Claude Code. A newline in a prompt is the `shift-enter` named key, which
  sends LF.

## GTK and crashes

- Never read or change a list model, expand a row, or change or destroy a widget inside
  that object's own signal handler (activate, notify, items-changed, gestures, drag and
  drop, menu actions). Copy out plain values and defer with `glib::idle_add_local_once`.
  Every segfault so far, in libgtk's tree-list code, broke this rule.
- `crates/xenon_eventcheck` is the commit check for that rule. It flags direct model and
  widget changes inside `connect_*` closures. Put `// sync: <reason>` on that line or
  the line above to mark one as safe.
- There is no crash handler. If an instance vanishes, check the slot's `xenon.log` for a
  panic, since stderr goes there. A SIGSEGV or abort leaves no line. The launcher turns
  core dumps on, so `coredumpctl info PID` shows the stack.

## Other traps

- `.vscode/tasks.json` has only the `xenon` and `install` tasks. Build, format and lint
  run through cargo or `scripts/*`. Do not add task labels. The pre-commit hook is the
  commit check, and `scripts/health` is the full run.
- GtkSourceView's syntax tags are anonymous. Code that copies them has to match on how
  they look, not on the tag name.
- Markdown YAML metadata needs the pulldown-cmark metadata option. Without it, the
  closing fence can turn into a setext heading.
- Prose and Source are two views of one text buffer. `xenon_prose` holds the parser and
  the rule for which markup shows. The GTK tags are in `prose.rs`.
