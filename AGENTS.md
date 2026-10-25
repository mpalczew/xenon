# AGENTS.md

Xenon: a native macOS (and experimental Linux) agent shell in Rust on GPUI.
Workspaces sidebar; pane tree of terminals and tree-sitter editors.
GPL-3.0-or-later. Public repo `mpalczew/xenon`.

## Where truth lives

- Product vision, personas, priorities: `PRODUCT.md`. Visual design: `DESIGN.md`.
  Read both before chrome or UX work; update them when a decision settles.
- Cross-cutting decisions and gotchas: `docs/project-state.md`.
- Install, signing, Linux build deps: `README.md`, `scripts/release/`.
- Release checklist: `docs/release.md`.
- Keyboard map and focus model: `docs/keyboard-first.md`.
- Screenshot scenes: `docs/visual-tests.md`.
- Claude Code IDE protocol: `crates/xenon_ide/PROTOCOL.md`.
- Phone remote design: `docs/designs/mobile-remote-v2.md`.
- Copied/adapted source provenance: `ATTRIBUTION.md`.

## Build and check

- Plain cargo at the workspace root.
  Author sessions pass `--profile dogfood` to build/test/clippy/run.
  `scripts/health` is the full gate. The
  pre-commit hook (`scripts/install-hooks`) is the commit gate.

## Gotchas

- Typed text arrives via `EntityInputHandler`, not key-down; ancestors act
  only when their own handle is focused (`docs/project-state.md`).
- Linux: the launcher forks and exits, so a systemd unit needs
  `KillMode=process`.
- Never `pkill`/`killall` or bypass `~/bin/xenon`. Killing an instance killed
  every background agent in it.

## Rules

- Views draw, models decide: no product-state branching, ranking, parsing, or
  policy in render code.
- Unit tests: default zero new. Add one only for a recurring bug, a boundary
  format, or logic too tangled to check by eye. Never pin tunable policy.
- Painted UI is unfinished until you capture one scene per touched UI state and
  look at it (`docs/visual-tests.md`).
- Keyboard-first: every flow works from the keyboard (action, binding, focus,
  Escape). Mouse is additive.
- Copied source gets an `ATTRIBUTION.md` entry and a file-header marker.
- Author sessions: the agent reviews, builds, and tests; no human reads diffs.
  Prefer large coherent hauls.
- Finish: `XENON_PROFILE=dogfood scripts/release/install && scripts/prune-target`
  once at the end, then commit and push this session's paths. List leftover dirty paths and whose they are.
