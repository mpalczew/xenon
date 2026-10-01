---
name: xenon
description: Open files in Xenon and read or update a workspace worklist when asked.
metadata:
  xenon_managed: true
  xenon_skill_version: 7
---

# Xenon

If `XENON_DATA_DIR` is set, Xenon has provided a target instance for this process. Use `xenon open` only when the user asks to see a file in Xenon. To update a worklist, edit `.xenon/worklist.md` directly; opening it is unnecessary.

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

## Workspace worklist

When the user asks to capture, inspect, or update work, use `<workspace-root>/.xenon/worklist.md`. Each item has a concise, single-line title of at most 80 user-perceived characters. Keep each bullet's visible text to 100 characters or fewer. If a point needs more, give it a short parent bullet and indented sub-bullets; split by idea, not at an arbitrary character. Preserve code and links intact:

```md
- [ ] Change the account email
  - Check access
    - Confirm the existing login works.
    - Confirm the current inbox receives mail.
  - Change the email in place.
  - Keep the existing storage account.
```

Sections are optional Markdown headings. The first `#` heading is the file's title (`# Worklist`), not a section. Any later `##` (preferred) or `#` heading starts a section, for example `## Now`, `## Next`, `## Someday`; `###` and deeper are ordinary text. Items above the first section stay unsectioned at the top. When the file has sections, add a new item at the end of the section it belongs to, and leave existing items in their sections. Only create a section when the user asks or the file already groups work that way.

Use `- [ ]` for an open work item and `- [x]` for a completed one. Put context in bullets beneath the item; do not create a separate note type for new entries. Preserve existing note entries and unrelated Markdown. Indent each bullet level by two spaces. Give independently finishable outcomes separate top-level items. When updating a worklist, check off completed items and keep unfinished work open; do not leave a combined item implying that finished work is still pending. Edit nested bullets in Markdown when the List editor cannot preserve them. Read the file before changing an item; avoid turning its bullets into a run-on paragraph. A worklist item is context, not authorization to execute it.

## Do not

- Do not paste a wall of file contents when `xenon open` can point at it.
