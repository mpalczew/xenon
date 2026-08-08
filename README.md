# Xenon

**The anti-IDE for people who run agents.**

A fast, native macOS shell for driving coding agents across workspaces.
Any harness that speaks a terminal. No vendor religion. Chrome that gets
out of the way.

> Noble gas. Present, inert, not trying to be your whole stack.

**macOS 11+ · Rust / GPUI · GPL-3.0-or-later**

---

## Why

Most “AI IDEs” bolt a chat panel onto an editor and lock the session inside
the app. Terminal multiplexers give you panes and no project model. Cursor is
close in spirit—and locked to one harness.

Xenon is the missing layer:

| You use today | Gap |
|---------------|-----|
| Many terminal tabs / tmux | No workspaces, weak editor, no attention model |
| Cursor / VS Code AI / Studio | Strong single-flow UX; often one harness; heavier feel |
| Multiple IDE windows | Context soup; slow switch |

**Positioning:** multi-agent workbench between a multiplexer and an AI IDE—
native speed, harness-neutral, daily-driver shell.

---

## Anti-IDE (product rule, not slogan)

1. **The harness is primary.** Agents run in a real PTY. Start Claude Code,
   Grok, Kimi, Codex, or anything else in Xenon—continue the same session in
   Terminal.app or on another machine. The agent is free to leave.
2. **No harness religion.** Defaults never imply one model vendor. Zero special
   config for a stock Claude install; everything else that runs in a terminal
   works the same way.
3. **Chrome disappears.** Workspaces, tabs, status, keyboard. Not decorative AI
   chrome. Not Electron lag when you type.
4. **Familiar bones, different job.** File tree, cmd-p, splits, editor—enough to
   live here all day. The product is parallel agents across workspaces, not
   VS Code feature parity on day one.

Market this honestly: **anti-IDE shell where the agent is free to leave.**

---

## Features

- **Multi-workspace sessions** — project roots with saved layout and tabs
- **First-class terminals** — not a drawer; agents live as tabs (and splits)
- **Editors when you need them** — tree-sitter highlighting, find, vim mode,
  LSP for Rust and TypeScript
- **Pane tree** — horizontal/vertical splits; drag tabs to move or split
- **Cmd-p family** — file finder, command palette, task runner
- **Keyboard-first** — every daily flow works without a mouse
- **Claude Code IDE bridge** — optional open-file / workspace hooks via local MCP
- **Themes** — dense dark defaults; True Black opt-in
- **Native** — Rust on GPUI (Zed’s UI stack). Instant feel is a product requirement

---

## Install

Homebrew is planned. Today you build from source (release build + app bundle).

### Prerequisites

| Tool | Notes |
|------|--------|
| **macOS** | 11.0+ |
| **Rust** | stable (`rustup`); edition 2024 |
| **cmake** | `brew install cmake` |
| **Xcode Metal Toolchain** | `xcodebuild -downloadComponent MetalToolchain` |
| **Xcode CLT** | usual macOS build tools |

Missing `cmake` or the Metal toolchain fails inside `wasmtime` / `gpui_macos`.

### Build and install

```bash
git clone https://github.com/mpalczew/xenon.git
cd xenon
./scripts/release/install
open ~/Applications/Xenon.app
```

That release-builds `xenon`, assembles `Xenon.app`, **stable-codesigns** it
(so TCC grants like Full Disk Access survive reinstalls), and copies it to
`~/Applications` (not `/Applications`—reinstalling there from a shell hosted
by the app trips macOS App Management prompts).

Override install location:

```bash
XENON_INSTALL_DIR=/path/to/dir ./scripts/release/install
```

Signing identity (first match wins): `XERO_CODESIGN_IDENTITY` if set, else
Developer ID Application, else Apple Development, else a local `xero-dev`
cert. Inspect with:

```bash
./scripts/release/sign_setup
```

Do **not** ad-hoc sign with `-` for daily installs—the CDHash changes every
build and macOS drops permissions.

### Dev loop

```bash
cargo run -p xenon          # debug
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
scripts/health              # full gate: fmt + clippy + shape + test
```

Bundle only (no copy to Applications):

```bash
./scripts/release/bundle
# → target/release/xenon.app
```

---

## First minutes

1. Open a **workspace** (project / checkout root).
2. **⌘N** — new terminal. Run your agent: `claude`, `grok`, `codex`, whatever.
3. More terminals or workspaces for parallel streams—not more Electron windows.
4. **⌘P** open files; **⌘⇧P** commands; split when a stream needs an editor.
5. Leave. Come back. Session layout is yours; the agent session belongs to the
   harness, not Xenon.

---

## Keyboard (daily)

| Action | Keys |
|--------|------|
| New terminal / new file | ⌘N / ⌘⇧N |
| Open workspace / file | ⌘⇧O / ⌘P |
| Command palette / tasks | ⌘⇧P / ⌘⇧R |
| Focus terminal / editor / files | ⌘1 / ⌘2 / ⌘3 |
| Workspace next / previous | ⌘⌥↓ / ⌘⌥↑ |
| Tab next / previous / close | ⌃⇥ / ⌃⇧⇥ / ⌘W |
| Save | ⌘S |

Full map: [`docs/keyboard-first.md`](docs/keyboard-first.md).

---

## Status

Xenon is **early, public, and daily-driven** by its author. Expect sharp edges.
Attention/status for “which agent needs you” is still maturing—the shell and
multi-harness path are the wedge.

**Success metric we care about:** substantive issues and PRs from people who
actually run agents this way—not vanity stars.

---

## Stack

| Piece | Role |
|-------|------|
| `xenon` | App binary, window, menus |
| `xenon_ui` | Shell: workspaces, panes, palettes, chrome |
| `xenon_core` | Pure session / workspace model |
| `xenon_store` | `~/.xenon` JSON persistence |
| `xenon_terminal` | PTY + terminal view (Zed terminal backend) |
| `xenon_editor` | Rope buffer + tree-sitter editor |
| `xenon_finder` | Ignore-respecting walk + fuzzy match |
| `xenon_ide` | Claude Code local IDE / MCP bridge |
| `xenon_lsp` | stdio LSP host (Rust, TypeScript) |

Product vision: [`PRODUCT.md`](PRODUCT.md). Visual system: [`DESIGN.md`](DESIGN.md).

---

## Contributing

Issues and PRs that improve the multi-agent daily path are welcome. Prefer
signal over volume—low-effort generated noise costs more than it helps.

**Before you open a PR:** run `scripts/health` (or `cargo test` / `fmt` /
`clippy` as in the README install section). Read [`CONTRIBUTING.md`](CONTRIBUTING.md)
for the full bar, and [`PRODUCT.md`](PRODUCT.md) before chrome or positioning
changes.

---

## License

[GPL-3.0-or-later](LICENSE) (SPDX). The `LICENSE` file is the GNU GPL version 3;
Cargo and this project allow “or any later version.”

Parts adapted from other projects are tracked in [`ATTRIBUTION.md`](ATTRIBUTION.md).

---

**Xenon** — present but inert. The anti-IDE for people who run agents.
