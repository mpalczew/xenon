# Xenon

**The anti-IDE for people who run agents.**

A fast, native Linux shell for driving coding agents across workspaces. Any harness that
speaks a terminal. No vendor religion. Chrome that gets out of the way.

> Noble gas. Present, inert, not trying to be your whole stack.

**Linux · Rust · GPL-3.0-or-later**

---

## Why

Most “AI IDEs” bolt a chat panel onto an editor and lock the session inside the app.
Terminal multiplexers give you panes and no project model. Cursor is close in spirit—and
locked to one harness.

Xenon is the missing layer:

| You use today                | Gap                                                    |
| ---------------------------- | ------------------------------------------------------ |
| Many terminal tabs / tmux    | No workspaces, weak editor, no attention model         |
| Cursor / VS Code AI / Studio | Strong single-flow UX; often one harness; heavier feel |
| Multiple IDE windows         | Context soup; slow switch                              |

**Positioning:** multi-agent workbench between a multiplexer and an AI IDE— native
speed, harness-neutral, daily-driver shell.

---

## Anti-IDE (product rule, not slogan)

1. **The harness is primary.** Agents run in a real PTY. Start Claude Code, Grok, Kimi,
   Codex, or anything else in Xenon—continue the same session in another terminal or on
   another machine. The agent is free to leave.
2. **No harness religion.** Defaults never imply one model vendor. Zero special config
   for a stock Claude install; everything else that runs in a terminal works the same
   way.
3. **Chrome disappears.** Workspaces, tabs, status, keyboard. Not decorative AI chrome.
   Not Electron lag when you type.
4. **Familiar bones, different job.** File tree, ctrl-p, splits, editor—enough to live
   here all day. The product is parallel agents across workspaces, not VS Code feature
   parity on day one.

Market this honestly: **anti-IDE shell where the agent is free to leave.**

---

## Features

- **Multi-workspace sessions** — project roots with saved layout and tabs
- **First-class terminals** — not a drawer; agents live as tabs (and splits)
- **Editors when you need them** — GtkSourceView highlighting, find, vim mode
- **Pane tree** — horizontal/vertical splits; drag tabs to move or split
- **Ctrl-p family** — file finder, command palette, task runner
- **Keyboard-first** — every daily flow works without a mouse
- **Phone remote** — Ctrl+Shift+M shows a QR code; drive your agents' terminals from
  your phone over [Tailscale](https://tailscale.com) or Headscale (tailnet only, never
  your Wi-Fi; installable web app, special-key row, terminal re-wraps to phone width)
- **Themes** — dense dark defaults; True Black opt-in
- **Native** — Rust on GTK 4, libadwaita and VTE. Instant feel is a product requirement

---

## Install

Linux only (Ubuntu 24.04+, Wayland or X11). Today you build from source.

### Prerequisites

| Tool            | Notes                                                  |
| --------------- | ------------------------------------------------------ |
| **Rust**        | stable (`rustup`); edition 2024                        |
| **Build tools** | `build-essential cmake clang lld pkg-config` and so on |
| **GTK 4 stack** | GTK 4, libadwaita, VTE and GtkSourceView 5 (dev files) |

### Build and install

```bash
sudo apt install build-essential cmake clang lld pkg-config curl git rsync \
  libgtk-4-dev libadwaita-1-dev libvte-2.91-gtk4-dev libgtksourceview-5-dev \
  libssl-dev libzstd-dev libsqlite3-dev libgit2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust stable
git clone https://github.com/mpalczew/xenon.git && cd xenon
./scripts/release/install
```

Installs the GTK app as `~/.local/share/xenon/xenon-bin`, the slot launcher as
`~/.local/bin/xenon` (and `~/bin/xenon` when that directory exists), and a `.desktop`
entry with an icon. `XENON_APP_DIR` and `XENON_BIN_DIR` override. Update with `git pull
&& ./scripts/release/install`. Quit is `ctrl-q`.

Start Xenon with `xenon`. State lives in two slot directories, `~/.xenon-a` and
`~/.xenon-b`. With one instance running, `xenon` copies its state to the other slot and
starts a new instance there, so a fresh build runs beside the old one. `xenon status`
lists the instances. `xenon <dir>` and `xenon open path[:line[:col]]` go to the running
instance. The phone remote and its paired devices are shared (`~/.xenon-shared`).

### Dev loop

```bash
cargo run -p xenon_gtk      # debug
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
scripts/health              # full gate: fmt + clippy + shape
```

---

## First minutes

1. Open a **workspace** (project / checkout root).
2. **Ctrl+N** — new terminal. Run your agent: `claude`, `grok`, `codex`, whatever.
3. More terminals or workspaces for parallel streams—not more Electron windows.
4. **Ctrl+P** open files; **Ctrl+Shift+P** commands; split when a stream needs an
   editor.
5. Leave. Come back. Session layout is yours; the agent session belongs to the harness,
   not Xenon.

---

## Keyboard (daily)

| Action                          | Keys                               |
| ------------------------------- | ---------------------------------- |
| New terminal / new file         | Ctrl+N / Ctrl+Shift+N              |
| Open workspace / file           | Ctrl+Shift+O / Ctrl+P              |
| Command palette / tasks         | Ctrl+Shift+P / Ctrl+Shift+R        |
| Focus terminal / editor / files | Ctrl+1 / Ctrl+2 / Ctrl+3           |
| Workspace next / previous       | Ctrl+Alt+↓ / Ctrl+Alt+↑            |
| Tab next / previous / close     | Ctrl+Tab / Ctrl+Shift+Tab / Ctrl+W |
| Save                            | Ctrl+S                             |
| Connect phone (QR)              | Ctrl+Shift+M                       |

Full map and how to rebind with `~/.xenon/keymap.json`:
[`docs/keyboard-first.md`](docs/keyboard-first.md#customizing-keys).

---

## Status

Xenon is **early, public, and daily-driven** by its author. Expect sharp edges.
Attention/status for “which agent needs you” is still maturing—the shell and
multi-harness path are the wedge.

**Success metric we care about:** substantive issues and PRs from people who actually
run agents this way—not vanity stars.

---

## Stack

| Crate            | Role                                          |
| ---------------- | --------------------------------------------- |
| `xenon_gtk`      | The app: window, panes, terminals, editor     |
| `xenon_core`     | Pure session / workspace / pane-tree model    |
| `xenon_store`    | `~/.xenon` JSON persistence, CLI, open socket |
| `xenon_settings` | Settings model                                |
| `xenon_keymap`   | Default key bindings and `keymap.json`        |
| `xenon_git`      | Git status and diff model                     |
| `xenon_finder`   | Ignore-respecting walk + fuzzy match          |
| `xenon_prose`    | Markdown prose mode                           |
| `xenon_remote`   | Phone remote server                           |

Product vision: [`PRODUCT.md`](PRODUCT.md). Visual system: [`DESIGN.md`](DESIGN.md).

---

## Contributing

Issues and PRs that improve the multi-agent daily path are welcome. Prefer signal over
volume—low-effort generated noise costs more than it helps.

**Before you open a PR:** run `scripts/health` (or `fmt` / `clippy` as in the README
install section). Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full bar, and
[`PRODUCT.md`](PRODUCT.md) before chrome or positioning changes.

---

## License

[GPL-3.0-or-later](LICENSE) (SPDX). The `LICENSE` file is the GNU GPL version 3; Cargo
and this project allow “or any later version.”

Parts adapted from other projects are tracked in [`ATTRIBUTION.md`](ATTRIBUTION.md).

---

**Xenon** — present but inert. The anti-IDE for people who run agents.
