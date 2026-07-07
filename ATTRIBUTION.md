# Attribution

xero is licensed GPL-3.0-or-later. This file records the provenance of all
third-party source code **copied or adapted** into this repository, so that a
clean-room rewrite remains possible if licensing needs change. Plain cargo
dependencies (crates.io or git) are not listed; see `Cargo.toml`.

Every copied/adapted file also carries a header comment naming its entry here.

| Files | Source repo | Rev | License | Notes |
| ----- | ----------- | --- | ------- | ----- |
| `crates/xero_terminal/src/color.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | `convert_color` / named-ANSI mapping adapted from `crates/terminal_view/src/terminal_element.rs`. |
| `crates/xero_terminal/src/grid.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | Grid layout (row grouping, cell-to-TextRun, resize) adapted from `terminal_element.rs` `layout_grid`. |
| `crates/xero_terminal/src/view.rs` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | PTY spawn + subscribe patterns adapted from `terminal_view.rs` and `project/src/terminals.rs`. |
| `crates/xero_terminal/assets/one.json` | zed-industries/zed | ced90fc (v1.9.0) | GPL-3.0-or-later | Verbatim copy of zed's One theme family (`assets/themes/one/one.json`); loaded for One Light/Dark. |
