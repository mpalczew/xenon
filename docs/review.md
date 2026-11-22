# Review

Review the pushed commits. Report only real problems; say "No findings" if none.

Flag:

- Bugs: wrong behavior, panics, borrow conflicts across GTK callbacks, leaks of Rc
  cycles.
- GTK event rules (see crates/xenon_eventcheck).
- Code we didn't need: anything the platform, GTK, libadwaita or VTE already provides.
- Shortcuts defined outside crates/xenon/src/commands.rs.
- Comments that narrate code; tests (we don't write them).
- Settings or saved-file format changes that break existing user data.

Format: one bullet per finding: file:line, what breaks, how to trigger it.
