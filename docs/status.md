# Status

- App and crashes: healthy since the GTK event-rule fixes. `crates/xenon_eventcheck`
- Files sidebar: now built on GTK's `DirectoryList`, not yet tried by hand; moving a
  folder to another disk fails. `PRODUCT.md`
- Markdown prose mode: task items show a stray bullet; shading of blank lines in code
  blocks is fixed but not yet checked on screen; Enter inside a code block sometimes
  ends the block (cause unknown; suspected only inside a list item).
  `crates/xenon_prose`
- Phone remote: server rewritten (HTTP parsing, socket threads, keep-awake through GTK),
  not yet tried on a phone; the cursor always shows (accepted, VTE limit).
  `docs/designs/mobile-remote-v2.md`
- Keyboard: healthy. `crates/xenon/src/keys.rs`
