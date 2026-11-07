# Xenon

Linux agent shell in Rust on GTK 4, libadwaita and VTE. The GPUI app is reference-only
until it is deleted. GPL-3.0-or-later; public repo `mpalczew/xenon`.

## Layout

- `crates/xenon_gtk/` - the app.
- `crates/xenon`, `xenon_ui`, `xenon_terminal`, `xenon_editor`, `xenon_design_system`,
  `third_party/` - the GPUI app, reference only.
- Other `crates/` - shared crates with no GPUI; see `docs/designs/gtk-migration.md`.
- `PRODUCT.md`, `DESIGN.md` - product and visual design. Read both before UX work;
  update them when a decision settles.
- `docs/project-state.md` - cross-cutting decisions.
- `docs/keyboard-first.md` - keyboard map and focus model.
- `docs/designs/mobile-remote-v2.md` - phone remote.
- `docs/release.md` - release checklist.
- `ATTRIBUTION.md` - copied source; each copy gets an entry and a file-header marker.

## Commands

- `~/bin/xenon` - start Xenon. Never bypass it or use `pkill`/`killall`: killing an
  instance kills every agent in it.
- `scripts/worktree new NAME` - create a worktree.
- `scripts/health` - full check. The pre-commit hook (`scripts/install-hooks`) is the
  commit check.
- `scripts/release/install && scripts/prune-target` - once at the end, then commit and
  push this session's paths. List leftover dirty paths and whose they are.

## Rules

- Views draw, models decide: no product-state branching, ranking, parsing, or policy in
  render code.
- Prefer what the platform and our libraries already provide over our own code whenever
  possible. The less code we have, the better.
- No tests. Don't write or run any; build, pass the commit check, and hand the result to
  the user.
- Keyboard-first: every flow works from the keyboard (action, binding, focus, Escape).
  Mouse is additive.
