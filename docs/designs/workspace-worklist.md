# Workspace worklist

Each workspace has one worklist at `.xenon/worklist.md`. Code: `crates/xenon_worklist`
(parsing, changes, the file), `crates/xenon/src/worklist*` and
`crates/xenon/src/editor/list.rs` (desktop), `crates/xenon_phone/src/worklist.rs`
(phone).

## Decisions

- **Plain context, not a task runner.** Xenon never assigns or runs items, and a listed
  task isn't permission to do it. No IDs, dates or frontmatter.
- **Plain Markdown.** An item is a title line with indented point bullets; `- [ ]` makes
  it a task. The first `#` heading is the title; later `#` or `##` headings are
  sections. Existing text is rewritten only when that item is edited.
- **One write path.** The list, capture and the phone change the worklist only through
  `xenon_worklist::apply`, which checks the file hasn't changed since it was read and
  writes by temp file and rename. The editor tab's Save refuses if the file changed on
  disk and keeps your text.
- **Items are named by content.** An item's key is a hash of its Markdown (checkbox
  ignored) plus its count among identical items. Edits elsewhere, like an agent
  appending, don't disturb it, so a phone view minutes old still works. An op on an item
  edited elsewhere reports "gone" and writes nothing.
- **The task list leaves alone what it can't safely rewrite** (nested checkboxes, code,
  tables) and says to edit the Markdown.
- **Phone: view, check, add and delete.** Details, moves and sections stay on the
  desktop. Undo of a phone delete is the last one per workspace, kept in memory, and
  fails if the file changed since. The phone re-fetches on open and on wake; the desktop
  list reloads on file change and window focus.
- **Out of scope:** due dates, priorities, owners, sync, and search across workspaces.
