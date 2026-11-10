# Status

- App and crashes: healthy since the GTK event-rule fixes. `crates/xenon_eventcheck`
- Files sidebar: now built on GTK's `DirectoryList`, not yet tried by hand; moving a
  folder to another disk fails. `PRODUCT.md`
- Markdown prose mode: healthy; task items show a stray bullet. `crates/xenon_prose`
- Phone remote: server rewritten (HTTP parsing, socket threads, keep-awake through GTK),
  not yet tried on a phone; the cursor always shows (accepted, VTE limit).
  `docs/designs/mobile-remote-v2.md`
- Keyboard: healthy. `crates/xenon/src/keys.rs`
