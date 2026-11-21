# Status

- App and crashes: healthy since the GTK event-rule fixes. `crates/xenon_eventcheck`
- Files sidebar: now built on GTK's `DirectoryList`, not yet tried by hand; moving a
  folder to another disk fails. `PRODUCT.md`
- Markdown prose mode: task items show a stray bullet; shading of blank lines in code
  blocks is fixed (also inside list items) but not yet checked on screen; Enter inside a
  code block sometimes ends the block (cause unknown; suspected only inside a list
  item). `crates/xenon_prose`
- Phone remote: its own process, `xenon-phone.service` (works with no window open; each
  phone has its own scroll and size); built, not yet tried on a phone.
  `docs/designs/mobile-remote-v2.md`
- Keyboard: healthy. Zoom now follows the active tab (terminal, code editor, markdown);
  not yet tried by hand. `crates/xenon/src/keys.rs`
- Terminal holders: each terminal is its own `xenon-term hold` process and systemd unit,
  so closing the window or installing a new build keeps agents running; older builds are
  marked in the header bar and on tabs. Built, not yet used daily; the protocol is
  untested across a real upgrade. `docs/designs/terminal-holders.md`
- Review CI: Claude Sonnet reviews every code push to main and comments on the commit
  only when it finds something. `docs/review.md`
