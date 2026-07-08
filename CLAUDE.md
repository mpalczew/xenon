# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

xero: a macOS IDE for driving coding agents, built in Rust on GPUI (zed's UI
framework). Left sidebar of workspaces each holding multiple "streams of work"
(terminal + editors + layout sessions); main panel is an embedded terminal
optionally split with tree-sitter-highlighted editors. License: GPL-3.0.

## Build & run

- Dev: `cargo build` / `cargo run` (workspace root). `cargo test` runs the unit
  tests (pure crates only; GPUI views are verified by launch tests).
- Install as an app: `project install` (from `.project.sh`) release-builds,
  assembles `target/release/xero.app` (Info.plist in `macos/`, ad-hoc signed for
  Apple Silicon), and copies it to `/Applications`. `project bundle` stops before
  copying. Use these rather than re-running the steps by hand.
- Prerequisites beyond Rust >= 1.85: `cmake` (brew) and the Xcode Metal
  Toolchain (`xcodebuild -downloadComponent MetalToolchain`). Missing either
  fails the build inside `wasmtime-c-api-impl` / `gpui_macos` respectively.
- zed crates (gpui, gpui_platform, terminal, theme, settings, …) are git
  dependencies pinned to one zed rev in the root Cargo.toml. Never float the rev;
  upgrade all pins together, including the mirrored `[patch.crates-io]` entries
  (zed's patches do not propagate to consumers).

## Architecture

Seven crates under `crates/`:
- `xero` — bin: gpui `Application`, window, quit/menu wiring.
- `xero_ui` — the app shell: `XeroApp` root view (collapsible workspace sidebar,
  terminal/editor main panel, cmd-p `FinderView`), keybindings, actions.
- `xero_core` — pure data model (Workspace/Stream/SessionState). `Stream::
  working_dir()` is the seam that keeps worktree-backed streams a future additive
  change; `Backing` is a serde-tagged enum for the same reason.
- `xero_store` — JSON persistence under `~/.xero` (atomic writes, corrupt-file
  backup). `XERO_DATA_DIR` overrides the location (used by tests).
- `xero_terminal` — zed terminal backend + adapted `TerminalElement`; `init()`
  installs the zed globals (settings/theme/release_channel) the terminal needs.
- `xero_editor` — ropey `Buffer` (edit ops, save, mtime external-change) + gpui
  `EditorView`. Tree-sitter highlighting is still TODO.
- `xero_finder` — cmd-p: ignore-respecting walk + nucleo fuzzy match.
- `xero_ide` — Claude Code IDE integration: a localhost WebSocket MCP server
  (`~/.claude/ide/<port>.lock` + `CLAUDE_CODE_SSE_PORT` injected into terminals)
  that lets agents open files in xero. Protocol captured in
  `crates/xero_ide/PROTOCOL.md`.

Input pattern (learned, load-bearing): on macOS, plain typed text arrives through
an `EntityInputHandler` registered during paint (`window.handle_input`), NOT
through `on_key_down` — which only carries control/navigation keys. The terminal,
editor, and finder all follow this split. GPUI repaints on explicit `cx.notify()`;
the terminal subscribes to `terminal::Event::Wakeup` to repaint on PTY output.

Any source copied/adapted from another project (e.g. zed's terminal_view) must
get a provenance entry in `ATTRIBUTION.md` plus a file-header marker — this
keeps a clean-room rewrite possible.

## Rules

`.cursor/rules/core.mdc` holds the user's core AI rules (a copy of their
global `~/.claude/CLAUDE.md`, sourced from personalfiles
`doc/ai_rules/core.md`). Those rules apply here; do not duplicate them in this
file. Key points: minimize cognitive load, present options at real decision
forks before building, Rule of 7, never amend/rebase/force-push, bash not zsh.
- When constructing an implementation plan for this repo, include local QA via
  `project install` so the installed `/Applications/xero.app` is verified, not
  just the debug build.
- When implementation work is done, offer to commit and push. Do not commit or
  push without the user's explicit request.
