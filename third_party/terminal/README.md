# terminal

Vendored from [zed-industries/zed](https://github.com/zed-industries/zed)
`crates/terminal` at `ced90fc` (v1.9.0), GPL-3.0-or-later (`LICENSE-GPL`).
Provenance: `ATTRIBUTION.md`.

Xenon change (marked `Xenon` in `src/alacritty.rs`, plus small hooks in
`src/terminal.rs`): `Terminal::remote_handle()` returns a `Send + Sync`
`RemoteHandle` (write to PTY, named keystroke honoring app-cursor mode, grid
snapshot under the term lock, and a wake hook fired on the PTY thread when
output arrives). The phone remote uses it so typing and screen frames skip
the GPUI main thread.

On a zed pin bump, replace this tree with that revision's `crates/terminal`,
keep this manifest's explicit dependency versions in sync with the pin, and
re-apply the `remote_handle` patch.
