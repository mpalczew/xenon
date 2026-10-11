# gpui_linux

Vendored from [zed-industries/zed](https://github.com/zed-industries/zed)
`ced90fc` (v1.9.0), Apache-2.0. License text: `LICENSE-APACHE`. Provenance:
`ATTRIBUTION.md`.

Xenon change, in `src/linux/x11/client.rs`: the XDG desktop portal appearance
and button-layout callbacks snapshot the window list and drop the `X11Client`
`RefCell` borrow before calling into the windows. Those callbacks read
appearance back through `with_common`, which panics (`RefCell already
borrowed`) if the client state is still held. That happens on startup
whenever `xdg-desktop-portal` answers on X11.

On a zed pin bump, replace this tree with that revision's `crates/gpui_linux`,
keep this manifest's explicit dependency versions in sync with the pin, and
re-apply the snapshot.
