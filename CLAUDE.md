# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

xero: a macOS IDE for driving coding agents, built in Rust on GPUI (zed's UI
framework). Left sidebar of workspaces each holding multiple "streams of work"
(terminal + editors + layout sessions); main panel is an embedded terminal
optionally split with tree-sitter-highlighted editors. License: GPL-3.0.

## Build

- `cargo build` / `cargo run` (workspace root). `cargo test` runs unit tests
  (pure crates only; GPUI views are verified by manual demos).
- Prerequisites beyond Rust >= 1.85: `cmake` (brew) and the Xcode Metal
  Toolchain (`xcodebuild -downloadComponent MetalToolchain`). Missing either
  fails the build inside `wasmtime-c-api-impl` / `gpui_macos` respectively.
- zed crates (gpui, gpui_platform, terminal) are git dependencies pinned to one
  zed rev in the root Cargo.toml. Never float the rev; upgrade all pins
  together, including the mirrored `[patch.crates-io]` entries (zed's patches
  do not propagate to consumers).

## Architecture

Seven crates under `crates/`: `xero` (bin: gpui app shell, keymap), `xero_core`
(pure data model: Workspace/Stream/SessionState; `Stream::working_dir()` is the
seam that keeps worktree-backed streams a future additive change), `xero_store`
(JSON persistence under `~/.xero`), `xero_terminal` (zed terminal backend +
adapted TerminalElement), `xero_editor` (ropey + tree-sitter), `xero_finder`
(cmd-p: ignore walk + nucleo), `xero_ui` (sidebar, panes, file tree, zed
settings/theme init).

Any source copied/adapted from another project (e.g. zed's terminal_view) must
get a provenance entry in `ATTRIBUTION.md` plus a file-header marker — this
keeps a clean-room rewrite possible.

## Rules

`.cursor/rules/core.mdc` holds the user's core AI rules (a copy of their
global `~/.claude/CLAUDE.md`, sourced from personalfiles
`doc/ai_rules/core.md`). Those rules apply here; do not duplicate them in this
file. Key points: minimize cognitive load, present options at real decision
forks before building, Rule of 7, never amend/rebase/force-push, bash not zsh.
