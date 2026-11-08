# Attribution

Xenon is licensed GPL-3.0-or-later. This file records the provenance of all third-party
source code **copied or adapted** into this repository, so that a clean-room rewrite
remains possible if licensing needs change. Plain cargo dependencies (crates.io or git)
are not listed; see `Cargo.toml`.

Every copied/adapted file also carries a header comment naming its entry here.

| Files                                                                            | Source repo                                                            | Rev                                                                                          | License          | Notes                                                                                                                |
| -------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ---------------- | -------------------------------------------------------------------------------------------------------------------- |
| `crates/xenon/src/theme.rs` (`terminal`: fg, bg, 16 ANSI colors, light and dark) | https://gitlab.gnome.org/chergert/ptyxis, `src/palettes/gnome.palette` | `aff963f90725089f09691d4dc88d2ee52d6d6103` (main when read; file last changed in `ea1a6da3`) | GPL-3.0-or-later | Ptyxis GNOME palette: values copied as data, Light and Dark variants; low-contrast entries adjusted (see `terminal`) |
