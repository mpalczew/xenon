# Status

- App and crashes: healthy since the GTK event-rule fixes. `crates/xenon_eventcheck`
- Files sidebar: now built on GTK's `DirectoryList`, not yet tried by hand; moving a
  folder to another disk fails. `PRODUCT.md`
- Markdown prose mode: task items show a stray bullet; shading of blank lines in code
  blocks is fixed (also inside list items) but not yet checked on screen; Enter inside a
  code block sometimes ends the block (cause unknown; suspected only inside a list
  item). `crates/xenon_prose`
- Phone remote: mostly healthy; its own process, `xenon-phone.service`, works with no
  window open, and closing a workspace from the phone works over `ipc.sock`. Scrollback
  is now native (the browser scrolls a server-held line window);
  `docs/designs/phone-scrollback.md`. Known issue: on iPadOS in Direct mode, navigating
  back and forth occasionally shortens a terminal to 8 rows (`fit` viewport diagnostics
  are live in the service journal to catch the next repro).
  `docs/designs/mobile-remote-v2.md`
- Worklist: shared `crates/xenon_worklist` crate with content-keyed ops; phone view
  (check, add, delete with undo) and the reworked desktop list and capture dialog are
  not yet tried by hand. `docs/designs/workspace-worklist.md`
- Keyboard: healthy. Zoom now follows the active tab (terminal, code editor, markdown);
  not yet tried by hand. `crates/xenon/src/commands.rs`
- Terminal holders: healthy and in daily use; each terminal is its own `xenon-term hold`
  process and systemd unit, so window restarts and installs keep agents running, and
  older builds are marked in the header bar and on tabs.
  `docs/designs/terminal-holders.md`
- Review CI: Claude Sonnet reviews every code push to main and comments on the commit
  only when it finds something. `docs/review.md`
