# Attribution

Xenon is licensed GPL-3.0-or-later. This file records the provenance of all third-party
source code **copied or adapted** into this repository, so that a clean-room rewrite
remains possible if licensing needs change. Plain cargo dependencies (crates.io or git)
are not listed; see `Cargo.toml`.

Every copied/adapted file also carries a header comment naming its entry here.

| Files                                         | Source repo                                                                | Rev                           | License                                                                                                                          | Notes                                                                                                                                                                                                                                                                                                                  |
| --------------------------------------------- | -------------------------------------------------------------------------- | ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/xenon_core/src/tasks/background.rs`   | microsoft/vscode                                                           | (problem matcher definitions) | MIT                                                                                                                              | The `$tsc-watch` `beginsPattern` / `endsPattern` regexes, copied from VS Code's built-in TypeScript problem matcher so a watch task's readiness matches VS Code.                                                                                                                                                       |
