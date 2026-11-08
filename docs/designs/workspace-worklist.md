# Workspace worklist

Each workspace has one worklist, a Markdown file at
`<workspace-root>/.xenon/worklist.md`. It is plain context for people and agents. Xenon
does not assign, dispatch or run items, and a listed task is never permission to execute
it. There is no frontmatter, ID, date, database or agent protocol.

## Using it

- Ctrl+Shift+K (Add to Worklist) opens a quick capture popover. Ctrl+Alt+K (Open
  Worklist) opens or focuses the ordinary, closable worklist editor tab. Closing the tab
  keeps the file. With no workspace, both explain that one is needed.
- Capture edits the same item form as the tab: a title and one point per line (Enter
  adds a point, Tab indents). It saves as you type once there is a title. Escape,
  Ctrl+Return or an outside click closes it. Ctrl+Backspace deletes the item. A saved
  item shows a toast with Undo. Unsaved text stays for that workspace until the app
  closes.
- When the file has sections, capture shows "Into Section", cycled with Alt+Up and
  Alt+Down. It starts on the last section used in this session. The item is appended at
  the end of that section. It locks once the first save lands.
- The tab opens in List mode, with a List | Markdown toggle. The list shows sections as
  headings (rename and delete from a menu), items as rows with a check button, the
  title, and the points joined as a subtitle, and an "Add item" row at the end of each
  section. Done items stay in place, dimmed, unless Hide Completed is on. Move to
  Section appends an item to another section. Items cannot be reordered. Deletes show an
  Undo toast.
- Markdown mode is the normal editor over the same buffer. Switching back to List needs
  the Markdown edits saved or undone first. An item the list cannot safely rewrite
  (nested checkboxes, code, tables) stays visible and says to use Markdown.

## File format

- An item is a title line with indented point bullets. A `- [ ]` checkbox makes it a
  task; without one it is a note. Titles are one line and aim for 80 characters; longer
  ones save, with a warning in the editor. Keep points short, and use child bullets for
  longer ideas. The list flattens nested points into the subtitle, so nested items are
  edited in Markdown.
- Sections are plain headings. The first `#` heading is the document title and is not a
  section. Any later `#` or `##` starts one (`##` preferred); `###` and below are
  ordinary content. Items before the first section show at the top with no heading.
- Existing Markdown stays valid and is rewritten only when that item is edited. Edits
  keep the file's line endings and leave unrelated text alone. Hard-wrapped lines belong
  to the title or point above them.
- An absent file opens as an empty unsaved document. The directory and file are created
  on the first write.

## Writing safely

- Capture, list actions and Markdown Save all go through one validated path with an
  exact-bytes revision check, and write by temporary file and rename. If the file
  changed on disk, the write is refused and your text stays available. `:w!` cannot
  override that.
- When the worklist tab is open, capture writes through the tab's buffer so there is one
  writer. If the tab has unsaved Markdown edits, resolve them first.
- Clean open tabs poll for outside changes, so agent edits show up without refocusing.
- Symlinks that leave the workspace are rejected. An unclosed code fence or frontmatter
  blocks capture with an error.
- Without file locking, a writer racing between the last check and the rename can still
  win.

## Out of scope

Due dates, priorities, owners, background runners, sync, and search across workspaces.
The Xenon-installed agent skill describes this format and tells agents to add concise
titles and points and to preserve unrelated entries.
