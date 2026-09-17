---
name: xenon
description: Open files in the Xenon agent shell beside the terminal with xenon open.
metadata:
  xenon_managed: true
  xenon_skill_version: 1
---

# Xenon

You are running inside Xenon, a native macOS agent shell. The PTY is yours. Use Xenon to show the user code instead of dumping files in chat.

## Open a file

```
xenon open path/to/file.rs:42
xenon open path/to/file.rs:42:8
xenon open path/to/file.rs:42-80 --pane sibling
```

- Location is 1-based. `:line`, `:line:col`, or `:start-end`.
- `--pane sibling` (default) opens in the other pane, or splits right if there is only one. Does not steal terminal focus.
- `--pane focused` tabs into the current pane.
- `--pane split-right` always splits right when the nest cap allows.

If `$XENON_DATA_DIR` is set, `xenon open` talks to this window even when another Xenon slot is running.

## Do not

- Do not paste a wall of file contents when `xenon open` can point at it.
- Do not pass slot names. The parent terminal already selected the instance.
