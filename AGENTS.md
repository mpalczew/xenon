# Xenon

Linux agent shell in Rust on GTK 4, libadwaita and VTE. GPL-3.0-or-later; public repo
`mpalczew/xenon`.

## Layout

- `crates/xenon/` - the app.
- Other `crates/` - shared crates with no GTK: rules, state and parsing.
- `PRODUCT.md` - what Xenon is and does. Read it before UX work.

## Commands

- `~/.local/bin/xenon` - start Xenon. Never bypass it or use `pkill`/`killall`: killing
  an instance kills every agent in it.
- `scripts/health` - full check.
- `scripts/release/install` - at the end, after you commit and push.

## Rules

- Prefer what the platform and our libraries already provide over our own code whenever
  possible. The less code we have, the better.
- No tests. Don't write or run any; build, pass the commit check, and hand the result to
  the user.
