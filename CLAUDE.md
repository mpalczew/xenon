# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Xenon: a macOS agent shell for driving coding agents, built in Rust on GPUI
(zed's UI framework). Left sidebar of workspaces; main panel is an embedded
terminal optionally split with tree-sitter-highlighted editors (tabs per
workspace). License: GPL-3.0-or-later. Public repo: `mpalczew/xenon`.

**Product vision (source of truth: `PRODUCT.md`):** native macOS **agent shell**
(**anti-IDE**) for many agents across workspaces, any terminal harness
(PTY-agnostic). "IDE" means live-here-all-day (author replaced VS Code), not
VS Code parity on day one. Public name **Xenon**; CLI/binary **`xenon`**;
app `Xenon.app`; data `~/.xenon` (migrates from `~/.xero`). Public install:
`./scripts/release/install` → `~/Applications/Xenon.app` (see README). Author
dogfood path may use `project install` / `~/bin/xenon`. OSS success:
**substantive issues/PRs**. Chrome: Option 2 + 3-lite; True Black opt-in.
Personas and product priorities live in `PRODUCT.md`. Strangers: `README.md`
and `CONTRIBUTING.md`.

## Build & run

- Dev: run cargo directly — `cargo build` / `cargo run` / `cargo test` /
  `cargo fmt --all` / `cargo clippy --workspace --all-targets -- -D warnings`
  (workspace root). Unit tests cover pure crates only; GPUI views are verified
  by launch tests. Full gate: `scripts/health` (fmtcheck + clippy + shape + test).
- Project tasks (`.vscode/tasks.json` — only two; agents run everything else via
  cargo/scripts): `project xenon` launches via `~/bin/xenon`; `project install`
  release-builds, assembles `target/release/xenon.app` (Info.plist in `macos/`),
  **stable-codesigns** it, and copies it to `~/Applications/Xenon.app` (override
  with `XENON_INSTALL_DIR` or legacy `XERO_INSTALL_DIR`). Not `/Applications`:
  reinstalling there from a shell hosted by the app triggers macOS App
  Management TCC every time. `scripts/release/bundle` stops before copying.
  Signing identity (for TCC grants to survive reinstall): `XERO_CODESIGN_IDENTITY`
  if set, else first `Developer ID Application`, else `Apple Development`, else
  auto-created local `xero-dev` self-signed cert. Bundle id stays
  `dev.xero.xero` so TCC grants stick.
  Never ad-hoc `-` for install — that changes the CDHash every build and drops
  Full Disk Access. Use `scripts/release/sign_setup` to print the identity.
  Complex release steps live under `scripts/release/`.
- Prerequisites beyond Rust >= 1.85: `cmake` (brew) and the Xcode Metal
  Toolchain (`xcodebuild -downloadComponent MetalToolchain`). Missing either
  fails the build inside `wasmtime-c-api-impl` / `gpui_macos` respectively.
- zed crates (gpui, gpui_platform, terminal, theme, settings, …) are git
  dependencies pinned to one zed rev in the root Cargo.toml. Never float the rev;
  upgrade all pins together, including the mirrored `[patch.crates-io]` entries
  (zed's patches do not propagate to consumers).

## Architecture

Crates under `crates/`:
- `xenon` — bin: gpui `Application`, window, quit/menu wiring.
- `xenon_ui` — the app shell: `XenonApp` root view (workspace sidebar, terminal/
  editor main panel with tabs, cmd-p finder, Run Task palette), keybindings.
- `xenon_core` — pure data model (Workspace/SessionState). Session layout + open
  editors are keyed by workspace; tabs (terminals + editors) are the multi-surface.
- `xenon_store` — JSON persistence under `~/.xenon` (atomic writes, corrupt-file
  backup; migrates from `~/.xero`). `XENON_DATA_DIR` overrides the location
  (legacy `XERO_DATA_DIR` still works; used by tests).
- `xenon_terminal` — zed terminal backend + adapted `TerminalElement`; `init()`
  installs the zed globals (settings/theme/release_channel) the terminal needs.
- `xenon_editor` — ropey `Buffer` (edit ops, save, mtime external-change) + gpui
  `EditorView` with tree-sitter highlighting.
- `xenon_finder` — cmd-p: ignore-respecting walk + nucleo fuzzy match.
- `xenon_settings` — app settings + font size actions.
- `xenon_ide` — Claude Code IDE integration: a localhost WebSocket MCP server
  (`~/.claude/ide/<port>.lock` + `CLAUDE_CODE_SSE_PORT` injected into terminals)
  that lets agents open files in Xenon. Protocol captured in
  `crates/xenon_ide/PROTOCOL.md`.
- `xenon_lsp` — runtime-neutral stdio JSON-RPC client/host for Rust and
  TypeScript navigation, highlights, and diagnostics.
- `xenon_remote` — optional local HTTP service for phone-sized terminal
  viewport and input over LAN/Tailscale.
- Project tasks: only `xenon` and `install` in `.vscode/tasks.json` (Run Task
  palette cmd-shift-r; `project <label>` from any shell). Build/test/fmt/lint
  are plain cargo (or `scripts/health`); do not invent extra `project` labels.

Input pattern (learned, load-bearing): on macOS, plain typed text arrives through
an `EntityInputHandler` registered during paint (`window.handle_input`), NOT
through `on_key_down` — which only carries control/navigation keys. The terminal,
editor, and finder all follow this split. GPUI repaints on explicit `cx.notify()`;
the terminal subscribes to `terminal::Event::Wakeup` to repaint on PTY output.

Any source copied/adapted from another project (e.g. zed's terminal_view) must
get a provenance entry in `ATTRIBUTION.md` plus a file-header marker — this
keeps a clean-room rewrite possible.

## Rules

`.cursor/rules/core.mdc` holds the author's core AI rules when present. Those
rules apply to agent work here; do not duplicate them in this file. Key points:
minimize cognitive load, present options at real decision forks before
building, Rule of 7, never amend/rebase/force-push, bash not zsh.

- **Agent-owned quality on author sessions.** The product is open source and
  welcomes substantive human review via issues/PRs (`CONTRIBUTING.md`). On the
  author's solo agent sessions there is no second human in the loop: the agent
  reviews, builds, and tests; surface risks and decision forks, then implement.
  Prefer **large coherent hauls** over small PR-sized slices. Do not ask the
  author to read diffs for quality control.
- Run `project install` at the END of all work, once every change is complete —
  this is how the author dogfoods. It release-builds and installs
  `~/Applications/Xenon.app`; the author verifies there (or via `project xenon` /
  `~/bin/xenon`), not in the debug build. Public/contributor install path is
  `./scripts/release/install` (README). Do not install mid-way through a
  multi-step change; batch it as the final step. During work, use cargo /
  `scripts/health` directly — not `project` wrappers.
- When implementation work is done, offer to commit and push. Do not commit or
  push without the user's explicit request (except when an explicit goal
  requires landing public docs on `origin/main`).
- Product/design context: read `PRODUCT.md` (and `DESIGN.md` if present) before
  chrome or UX work. Keep them aligned with settled decisions; do not invent a
  competing vision in chat only. Cross-cutting current decisions live in
  `docs/project-state.md`; release work lives in `docs/release.md`.

## Keyboard-first (mouseless)

Every UX surface must be operable end-to-end from the keyboard. Mouse is additive,
never required for a complete flow.

Current bindings and the GPUI focus-ownership model live in
`docs/keyboard-first.md`.

- Design the keyboard path first: action, keybinding, focus target, Escape dismiss.
- Click-only chrome is incomplete. Lists, pickers, sidebars, dialogs, and empty
  states need arrows / Enter / Escape (and type-to-filter when lists are long).
- Prefer in-app palettes (cmd-p pattern) for frequent power-user flows; keep native
  macOS pickers as optional Browse… escape hatches, not the only path.
- Do not ship a mouse-only control when a keyboard equivalent exists elsewhere
  without wiring that equivalent (or adding one).
- When reviewing UX work, ask: can a user do this with hands on the home row?

## Never kill Xenon processes

Never run `pkill`, `killall`, or any command that could terminate running Xenon
instances or background agents without explicit user instruction. This includes:
- Never bypass `~/bin/xenon` (or `~/bin/xero` symlink; the slot launcher) to
  directly interact with `Xenon.app`
- Never run diagnostics on running processes without understanding side effects
- Always ask first if unsure whether a command could kill something

Context: I killed a running instance and all background agents by running
diagnostics without understanding the implications. This destroyed work in
progress. Use the slot launcher for all Xenon operations.
