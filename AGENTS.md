# Xenon

Linux agent shell in Rust on GTK 4, libadwaita and VTE. GPL-3.0-or-later; public repo
`mpalczew/xenon`.

## Layout

- `crates/xenon/` - the app.
- Other `crates/` - shared crates with no GTK: rules, state and parsing.
- `PRODUCT.md` - what Xenon is and does. Read it before UX work.
- `crates/xenon/src/keys.rs` - every app shortcut.
- `docs/designs/*` - selected design docs

## Commands

- `~/bin/xenon` - start Xenon. Never bypass it or use `pkill`/`killall`: killing an
  instance kills every agent in it.
- `scripts/worktree new NAME` - create a worktree.
- `scripts/health` - full check. The pre-commit hook (`scripts/install-hooks`) is the
  commit check.
- `scripts/release/install` - once at the end, then commit and push this session's
  paths. List leftover dirty paths and whose they are.

## Rules

- Prefer what the platform and our libraries already provide over our own code whenever
  possible. The less code we have, the better.
- No tests. Don't write or run any; build, pass the commit check, and hand the result to
  the user.
