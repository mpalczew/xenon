# Workspace worklist

The [single interactive mock](../../visual-review/workspace-queue.html) shows the agreed Good and Snazzy treatments in Xenon's current theme. Good is the implementation direction: two quiet toolbar actions, a quick capture popover, and a closable worklist tab. Snazzy illustrates a possible grouped treatment and unfinished-task count; that count is not part of this version.

## Experience

Each workspace has one worklist at `<workspace-root>/.xenon/worklist.md`. Nothing is permanently visible or automatically opened. The toolbar's **Add to Worklist** button (`⌘⇧K`) opens quick capture without opening a tab. **Open Worklist** (`⌘⌥K`) opens or focuses the same ordinary, closable editor tab, including one in another pane. Closing the tab keeps the file. A previously open tab may return through normal session restoration.

The two icon buttons sit together in the toolbar's right group after a divider. Their pointer and keyboard-focus hints show the command name and registered chord; icons without shortcuts show a name only. The command catalog drives labels and chords for the toolbar, command palette, and keyboard help. A toolbar action leaves the editor or terminal focused where appropriate. With no workspace, the worklist actions explain that a workspace is needed.

Toolbar capture shows the workspace name, one multiline task field, and Save. The field receives focus each time capture opens and whenever it is clicked. Enter submits, Shift+Enter inserts a line, and Escape or outside click dismisses while retaining the draft for that workspace during this app session. Empty text cannot be submitted. Successful capture clears the draft, closes the popover, restores the previous focus, and shows a dismissible notice with Undo that expires after four seconds. A failure keeps the draft and explains the error. Capture stays bound to the workspace where it began. If its worklist tab has unsaved Markdown changes, the user must resolve them before capture writes.

The worklist tab opens in **List** presentation by default. It reads the Markdown file directly and shows top-level checkbox tasks and existing standalone paragraph notes. **Add a task** opens one editor inline in the tab, with multiline text, Save, and Cancel. Enter saves, Shift+Enter inserts a line, and Escape cancels. The list also supports checkbox completion, selected-row keyboard navigation (↑/↓, Space, Enter), item editing, adjacent move, delete, and Undo/Redo. Item editing uses a multiline field; `⌘Enter` saves and Escape cancels. Move is limited to adjacent entries in the same section with no intervening Markdown content. A complex task that the list cannot safely rewrite remains visible; use **Markdown** for it. Markdown mode is the normal editor over the same buffer, with ordinary selection, find, Vim editing, dirty state, and Save. Switching back to List requires saving or undoing Markdown edits first. There is no duplicate task store or separate worklist window. Freeform notes can be written in Markdown mode or by an agent editing the file.

## File behavior

Opening an absent worklist creates an empty virtual document; it does not create `.xenon` or mark the tab dirty. Save is available in this empty state and creates the file with `# Worklist`. The directory and file are otherwise created on the first successful write. Task capture appends `- [ ] ` with indented continuation lines; Note capture appends plain Markdown. Captures preserve the file's line-ending style and do not reformat existing text. An unclosed code fence or frontmatter blocks capture with a recoverable error.

Capture, list actions, and Markdown Save share one validated file path and exact-byte revision check. Writes use a temporary file and atomic rename. A detected external change refuses the write and keeps local text available for reconciliation; `:w!` cannot bypass this for the worklist. Clean open tabs poll for external changes so agent edits appear without refocusing the tab. Symlinks that resolve outside the workspace are rejected. The revision check closes ordinary read–write conflicts but, without filesystem locking, cannot guarantee exclusion of a simultaneous writer between the last check and rename.

Agents interface with the worklist as an ordinary project file. They may read it, append a task or note, or edit/check off an item when instructed, preserving unrelated Markdown. Xenon does not infer ownership, dispatch tasks, or treat a listed task as authorization to execute it. The file needs no frontmatter, IDs, timestamps, proprietary metadata, database, or dedicated agent protocol.

## Verification and boundaries

Review capture (empty and filled), the List tab (populated, empty virtual, and inline Add in dark and light themes), item edit, Markdown mode, and toolbar hint in native visual scenes. Exercise storage conflicts, external edits, undo, keyboard input, and same-file saves in tests. Run `scripts/health` and native visual tests, then install with `project install`.

Due dates, priorities, ownership, background task runners, sync, search across workspaces, and the Snazzy count are outside this version. The mock demonstrates intended UX; native behavior and this spec define the implementation.
