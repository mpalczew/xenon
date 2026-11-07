# AGENTS.md

Xenon: a Linux agent shell in Rust on GTK 4, libadwaita and VTE (`crates/xenon_gtk`).
The GPUI app is reference-only until it is deleted. Workspaces sidebar; pane tree of
terminals and editors. GPL-3.0-or-later. Public repo `mpalczew/xenon`.

## Where truth lives

- Product vision, personas, priorities: `PRODUCT.md`. Visual design: `DESIGN.md`. Read
  both before chrome or UX work; update them when a decision settles.
- Cross-cutting decisions and gotchas: `docs/project-state.md`.
- Install and build deps: `README.md`, `scripts/release/`.
- Release checklist: `docs/release.md`.
- Keyboard map and focus model: `docs/keyboard-first.md`.
- Claude Code IDE protocol: `crates/xenon_ide/PROTOCOL.md`.
- Phone remote design: `docs/designs/mobile-remote-v2.md`.
- Copied/adapted source provenance: `ATTRIBUTION.md`.

## Build and check

- Plain cargo at the workspace root. `scripts/health` is the full gate. The pre-commit hook
  (`scripts/install-hooks`) is the commit gate.

## Gotchas

- Typed text arrives via `EntityInputHandler`, not key-down; ancestors act only when
  their own handle is focused (`docs/project-state.md`).
- Linux: the launcher forks and exits, so a systemd unit needs `KillMode=process`.
- Never `pkill`/`killall` or bypass `~/bin/xenon`. Killing an instance killed every
  background agent in it.

## Rules

- Views draw, models decide: no product-state branching, ranking, parsing, or policy in
  render code.
- Prefer what the platform and our libraries already provide over our own code whenever
  possible. The less code we have, the better.
- No tests. Don't write or run any; build, pass the commit check, and hand the result to
  the user.
- Keyboard-first: every flow works from the keyboard (action, binding, focus, Escape).
  Mouse is additive.
- Copied source gets an `ATTRIBUTION.md` entry and a file-header marker.
- Finish: `scripts/release/install && scripts/prune-target` once
  at the end, then commit and push this session's paths. List leftover dirty paths and
  whose they are.
