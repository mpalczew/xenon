# Contributing to Xenon

Thanks for caring enough to engage the codebase. Xenon is an early public
macOS **anti-IDE** shell for people who run coding agents. The success metric
is **substantive** issues and PRs—not vanity volume.

## What is welcome

- Fixes and features that improve the **multi-agent daily path**: workspaces,
  terminals, splits, session continuity, keyboard-first flows, latency.
- Clear bug reports with reproduction steps (macOS version, harness, what you
  ran, what you expected).
- Design-aligned UX work after reading [`PRODUCT.md`](PRODUCT.md) and
  [`DESIGN.md`](DESIGN.md).

## What is low-signal

Please skip or heavily curate:

- Unsolicited bulk refactors, drive-by dependency bumps, or “AI polish” diffs
  that do not fix a real problem.
- Issues that only restate the README without a reproduction.
- PRs that change product positioning or chrome system without engaging
  `PRODUCT.md` / `DESIGN.md`.
- Generated noise that the maintainer cannot review in one focused pass.

## Build and test

Public path (no private tooling required):

```bash
git clone https://github.com/mpalczew/xenon.git
cd xenon
# prereqs: Rust stable, cmake, Xcode Metal Toolchain — see README
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
scripts/health   # full gate: fmtcheck + clippy + shape + test
```

Install a release app bundle locally:

```bash
./scripts/release/install
open ~/Applications/Xenon.app
```

Details and signing notes: [`README.md`](README.md).

## Patch hygiene

- Prefer small, reviewable commits that do one thing.
- Match surrounding code (short functions, explicit over clever).
- Do not force-push shared history; merge, do not rebase published work.
- Run `scripts/health` (or the cargo equivalents above) before you open a PR.
- Leave intentional legacy identifiers alone unless you are sure: bundle id
  `dev.xero.xero`, `XERO_*` env aliases, `~/.xero` migration, and local
  `xero-dev` codesign name exist so TCC grants stick across renames.

## Communication

Open an issue for discussion when the change is large or product-shaped.
PRs that improve the multi-harness shell are the preferred form of help.
