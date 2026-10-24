# Attribution

Xenon is licensed GPL-3.0-or-later. This file records the provenance of all
third-party source code **copied or adapted** into this repository, so that a
clean-room rewrite remains possible if licensing needs change. Plain cargo
dependencies (crates.io or git) are not listed; see `Cargo.toml`.

Every copied/adapted file also carries a header comment naming its entry here.

| Files | Source repo | Rev | License | Notes |
| ----- | ----------- | --- | ------- | ----- |
| `crates/xenon_terminal/src/color.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | `convert_color` / named-ANSI mapping adapted from `crates/terminal_view/src/terminal_element.rs`. |
| `crates/xenon_terminal/src/grid.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | Grid layout (row grouping, cell-to-TextRun, resize) and the hovered-link underline adapted from `terminal_element.rs` `layout_grid`. |
| `crates/xenon_terminal/src/grid/row.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | Grid-anchored row segments shaped with a forced cell advance, adapted from `terminal_element.rs` batched text runs. |
| `crates/xenon_terminal/src/view/mod.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | PTY spawn + subscribe patterns adapted from `terminal_view.rs` and `project/src/terminals.rs`; Cmd-hover link detection drives zed's `Terminal::mouse_move`, and the grid position-to-cell mapping mirrors zed's `grid_point`. Xenon-only helpers live in `view/attention.rs`, `view/paths.rs` and `view/task_run.rs`. |
| `crates/xenon_terminal/assets/one.json` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | One theme family (`assets/themes/one/one.json`); One Dark surfaces set to pure black; One Light canvas/window retuned off the gray slab, and its syntax, dim text, line numbers, and terminal colors darkened to at least 4.5:1 on white. |
| `crates/xenon_terminal/assets/gruvbox.json` | zed-industries/zed | ced90fc (v1.9.0) | Apache-2.0 (via zed themes) | Gruvbox family (`assets/themes/gruvbox/gruvbox.json`); Dark and Light only (Soft/Hard variants dropped). |
| `crates/xenon_terminal/assets/solarized.json` | ethanschoonover/solarized | (palette) | MIT | Official Solarized palette mapped to zed theme schema; not copied from zed assets. |
| `crates/xenon_terminal/assets/nord.json` | arcticicestudio/nord | (palette) | MIT | Official Nord palette mapped to zed theme schema. |
| `crates/xenon_editor/queries/markdown/*.scm` | tree-sitter-grammars/tree-sitter-markdown (via crates.io `tree-sitter-md`) | 0.5.x stock queries | MIT | Block/inline highlights adapted from crate-shipped queries; injections add `include-children` for inline re-parse. |
| `crates/xenon_settings/fonts/lilex/*` | mishamyrt/Lilex (via zed `assets/fonts/lilex` at ced90fc) | OFL-1.1 | Bundled editor/terminal face. GPUI maps `.ZedMono` → Lilex; we load the TTFs so the picker resolves. |
| `crates/xenon_settings/fonts/ibm-plex-sans/*` | IBM Plex Sans (via zed `assets/fonts/ibm-plex-sans` at ced90fc) | OFL-1.1 | Bundled UI face. GPUI maps `.SystemUIFont` on Linux to IBM Plex Sans; we load the TTFs so chrome does not fall through to Lilex. |
| `third_party/gpui_linux/**` | zed-industries/zed `crates/gpui_linux` | ced90fc (v1.9.0) | Apache-2.0 | Vendored so the X11 portal callbacks can drop the `X11Client` borrow before calling back. Provenance header is on `src/linux/x11/client.rs` (the edited file) and `README.md`. |
| `third_party/terminal/**` | zed-industries/zed `crates/terminal` | ced90fc (v1.9.0) | GPL-3.0-or-later | Vendored so the phone remote can reach the PTY writer and term grid off the UI thread. Xenon change (`RemoteHandle`, `WakeHooks`, `Terminal::remote_handle`) is marked in `src/alacritty.rs` and described in `README.md`. |
| `crates/xenon_core/src/tasks/background.rs` | microsoft/vscode | (problem matcher definitions) | MIT | The `$tsc-watch` `beginsPattern` / `endsPattern` regexes, copied from VS Code's built-in TypeScript problem matcher so a watch task's readiness matches VS Code. |

Original Xenon theme families (no third-party source file): `neon`, `tokyo`,
`high_contrast`, `imperial`, `mithril`, `true_black`, `hotdog`.
