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
| `crates/xenon_terminal/src/view/mod.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | PTY spawn + subscribe patterns adapted from `terminal_view.rs` and `project/src/terminals.rs`; Cmd-hover link detection drives zed's `Terminal::mouse_move`, and the grid position-to-cell mapping mirrors zed's `grid_point`. Xenon-only helpers live in `view/attention.rs` and `view/paths.rs`. |
| `crates/xenon_terminal/assets/one.json` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | One theme family (`assets/themes/one/one.json`); One Dark surfaces set to pure black; One Light unchanged. |
| `crates/xenon_terminal/assets/ayu.json` | zed-industries/zed | ced90fc (v1.9.0) | MIT (via zed themes) | Ayu family (`assets/themes/ayu/ayu.json`); Dark / Light / Mirage. |
| `crates/xenon_terminal/assets/gruvbox.json` | zed-industries/zed | ced90fc (v1.9.0) | Apache-2.0 (via zed themes) | Gruvbox family (`assets/themes/gruvbox/gruvbox.json`); Dark/Light Soft/Hard variants. |
| `crates/xenon_terminal/assets/solarized.json` | ethanschoonover/solarized | (palette) | MIT | Official Solarized palette mapped to zed theme schema; not copied from zed assets. |
| `crates/xenon_terminal/assets/nord.json` | arcticicestudio/nord | (palette) | MIT | Official Nord palette mapped to zed theme schema. |
| `crates/xenon_editor/queries/markdown/*.scm` | tree-sitter-grammars/tree-sitter-markdown (via crates.io `tree-sitter-md`) | 0.5.x stock queries | MIT | Block/inline highlights adapted from crate-shipped queries; injections add `include-children` for inline re-parse. |

Original Xenon theme families (no third-party source file): `neon`, `ember`, `aurora`,
`abyss`, `tokyo`, `runner`, `ink`, `high_contrast`, `imperial`, `mithril`, `synthwave`,
`radioactive`, `hotdog`. IDE-familiar families (`vscode`, `intellij`, `xcode`) recreate
well-known editor palettes for migration; not copied from vendor assets.
