# Workspace worklist

Each workspace has one worklist at `.xenon/worklist.md`. Code: `crates/xenon_core`
(parsing and edits), `crates/xenon/src/worklist*` and `crates/xenon/src/editor/list.rs`.

## Decisions

- **Plain context, not a task runner.** Xenon never assigns or runs items, and a listed
  task isn't permission to do it. No IDs, dates or frontmatter.
- **Plain Markdown.** An item is a title line with indented point bullets; `- [ ]` makes
  it a task. The first `#` heading is the title; later `#` or `##` headings are
  sections. Existing text is rewritten only when that item is edited.
- **One safe write path.** Capture, the task list and Save all check the file hasn't
  changed since it was read, then write by temp file and rename. A changed file refuses
  the write and keeps your text. When the tab is open, capture writes through it, so
  there's one writer.
- **The task list leaves alone what it can't safely rewrite** (nested checkboxes, code,
  tables) and says to edit the Markdown.
- **Out of scope:** due dates, priorities, owners, sync, and search across workspaces.
