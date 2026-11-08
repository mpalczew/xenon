# Project state

Rules and traps that cut across the code. The code has the details.

Other docs:

- `PRODUCT.md`: what Xenon is and does.
- `docs/keyboard-first.md`: keys and focus.
- `docs/designs/mobile-remote-v2.md`: the phone remote.
- `docs/designs/vscode-tasks.md`: VS Code tasks.

## Running Xenon

- Xenon runs as two slots, A and B, each its own process with its own state folder
  (`~/.xenon-a`, `~/.xenon-b`). The launcher `~/bin/xenon` (`scripts/xenon`) picks the
  slot and copies state across when it switches.
- Never kill Xenon. Quitting it kills everything its terminals started. An install
  replaces the files, but a running Xenon keeps the old build until it is restarted.
- `xenon open FILE` opens the file in the workspace of the terminal it runs in. Each
  terminal gets `XENON_DATA_DIR` and `XENON_WORKSPACE` for this.

## GTK

- Inside a widget's or list model's own signal handler, never change that widget or
  model, expand a row, or destroy anything. Copy out plain values and defer the change
  with `glib::idle_add_local_once`. Every crash so far broke this rule.
- The commit check (`crates/xenon_eventcheck`) flags such changes in `connect_*`
  closures. Mark a safe one with `// sync: <reason>` on that line or the line above.
- GtkSourceView's syntax tags have no names. Code that copies them must match on how
  they look.

## Crashes

- A panic lands in the slot's `xenon.log`. A segfault leaves nothing there. Run
  `coredumpctl info PID` for its stack.

## Git

- Git runs with `--no-optional-locks`, so checking status never touches `.git/index` and
  sets off our own file watcher.
- Status refreshes when `.git` changes, when the window gets focus, and every 5 seconds
  while focused, since GTK can't watch a whole folder tree.

## Phone remote

- It listens only on the Tailscale address and loopback, never the local network,
  because it uses plain HTTP.
- On iOS, disabling a focused input drops focus for good. The page refocuses it on
  reconnect (`renderStale()`); new inputs that get disabled need the same.
- Text from the phone turns `\n` into `\r`, which submits in Claude Code. A newline
  inside a prompt is the `shift-enter` key.

## Markdown

- The Markdown and Code views share one text buffer. The parser and the rules for what
  markup shows are in `crates/xenon_prose`.
- YAML front matter needs pulldown-cmark's metadata option, or its closing `---` turns
  the line above into a heading.

## Tooling

- `.vscode/tasks.json` has only the `xenon` and `install` tasks. Don't add more.
- The pre-commit hook is the commit check. `scripts/health` runs everything.
