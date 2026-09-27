# Workspace worklist

The [single interactive mock](../../visual-review/workspace-queue.html) shows the title-and-bullets worklist in Xenon's light theme. The interaction uses two quiet toolbar actions, a quick capture popover, and a closable worklist tab. The item format below is implemented.

## Experience

Each workspace has one worklist at `<workspace-root>/.xenon/worklist.md`. Nothing is permanently visible or automatically opened. The toolbar's **Add to Worklist** button (`⌘⇧K`) opens quick capture without opening a tab. **Open Worklist** (`⌘⌥K`) opens or focuses the same ordinary, closable editor tab, including one in another pane. Closing the tab keeps the file. A previously open tab may return through normal session restoration.

The two icon buttons sit together in the toolbar's right group after a divider. Their pointer and keyboard-focus hints show the command name and registered chord; icons without shortcuts show a name only. The command catalog drives labels and chords for the toolbar, command palette, and keyboard help. A toolbar action leaves the editor or terminal focused where appropriate. With no workspace, the worklist actions explain that a workspace is needed.

Toolbar capture shows the workspace name, the shared outline, and Save. There is no Task/Note choice. Title receives focus each time capture opens. Enter adds a point, Tab indents, and ⌘Enter submits. Escape or outside click dismisses while retaining the draft for that workspace during this app session. An empty or overlong title cannot be submitted. Successful capture clears the draft, closes the popover, restores the previous focus, and shows a dismissible notice with Undo that expires after four seconds. A failure keeps the draft and explains the error. Capture stays bound to the workspace where it began. If its worklist tab has unsaved Markdown changes, the user must resolve them before capture writes.

The worklist tab opens in **List** presentation by default. It reads the Markdown file directly and shows top-level checkbox tasks, bullet notes, and existing standalone paragraph notes. **Add an item** opens the same editor as editing. Close leaves a saved item in the list. Delete removes it. There is no Save, Cancel, or list Undo button. The list supports checkbox completion, selected-row keyboard navigation (↑/↓, Space, Enter), item editing, and delete. Item editing is the outline; changes save as you type. Cmd-Z undoes typing in the focused field, and Cmd-Shift-Z puts it back. Escape or Close leaves the editor. Delete removes the item. There is no Task/Note choice and no move control. A complex task that the list cannot safely rewrite remains visible; use **Markdown** for it. Markdown mode is the normal editor over the same buffer, with ordinary selection, find, Vim editing, dirty state, and Save. Switching back to List requires saving or undoing Markdown edits first. There is no duplicate task store or separate worklist window. Freeform notes can be written in Markdown mode or by an agent editing the file.

## File behavior

Opening an absent worklist creates an empty virtual document; it does not create `.xenon` or mark the tab dirty. Save is available in this empty state and creates the file with `# Worklist`. The directory and file are otherwise created on the first successful write. Task capture appends `- [ ] Title` with indented detail bullets; Note capture appends `- Title` with the same detail bullets. Captures preserve the file's line-ending style and do not reformat existing text. An unclosed code fence or frontmatter blocks capture with a recoverable error.

Capture, list actions, and Markdown Save share one validated file path and exact-byte revision check. Writes use a temporary file and atomic rename. A detected external change refuses the write and keeps local text available for reconciliation; `:w!` cannot bypass this for the worklist. Clean open tabs poll for external changes so agent edits appear without refocusing the tab. Symlinks that resolve outside the workspace are rejected. The revision check closes ordinary read–write conflicts but, without filesystem locking, cannot guarantee exclusion of a simultaneous writer between the last check and rename.

Agents interface with the worklist as an ordinary project file. They may read it, append a task or note, or edit/check off an item when instructed, preserving unrelated Markdown. Xenon does not infer ownership, dispatch tasks, or treat a listed task as authorization to execute it. The file needs no frontmatter, IDs, timestamps, proprietary metadata, database, or dedicated agent protocol.

## Item format

The [mock](../../visual-review/workspace-queue.html) replaces run-on rows with a bold, scannable title and indented detail bullets. The Markdown convention is:

```md
# Worklist

- [ ] Change Cloudflare account email
  - Confirm password sign-in and access to the current inbox.
  - Change the login email in the existing account.
  - Rotate API tokens afterward.

- Account context
  - Keep the bucket in the existing account.
```

Every item has a nonempty, single-line title of at most 80 user-perceived characters. A checkbox distinguishes a task from a note; both take optional detail bullets. One idea or action goes in each bullet. No required labels, dates, IDs, or fixed bullet categories. Existing Markdown stays valid and is rewritten in this format only when the user edits that item. The list displays titles prominently and bullets beneath them, without concatenating them into a paragraph. Long bullet text may wrap; titles remain bounded.

Inline edit and quick capture focus a one-line Title field and expose an optional Details field that accepts one point per line, generating the indented Markdown bullets. Task is the default; changing to Note is one click. Saving requires a title. The title counter and limit use the same user-perceived character count in UI and validation. Editing a bullet must leave the title in place and preserve unrelated Markdown. Nested subtasks and complex Markdown blocks still require Markdown mode.

The Xenon-installed skill describes this exact file convention and tells agents to create or update items with concise titles and bullet details, preserving unrelated entries. It also clarifies that the worklist is context, not permission to execute a task.

## Proposed nested bullets and typography

Keep each bullet to at most 100 visible user-perceived characters. When an idea needs more room, use a short parent bullet with two-space-indented child bullets. Split by meaning rather than wrapping a sentence at the limit. The agent skill uses this file convention now; the native List view still flattens nested details, so nested items require Markdown mode until the list renderer and editor support them.

The [single mock](../../visual-review/workspace-queue.html) shows nested bullets in List, inline edit, quick capture, and Markdown states. Its typography specimen shows the approved roles for screen title, section title, body, list primary, supporting text, control label, and code. The native design-system typography API now owns their font family, size, weight, line height, and theme color.

The [single Snazzy editor mock](../../visual-review/workspace-queue.html) shows new-in-tab, quick-capture, and edit-existing contexts. The title is the first row in the canvas. Enter or Down from it enters the first point, creating that point if needed; Up from the first point returns to the title. The editor accepts plain text and serializes bullets to Markdown without typed markers or a Task/Note choice. Tab indents the focused point at any caret position. Backspace at its start outdents it, or merges a top-level point into the previous point. Existing note entries remain readable. The native editor is this outline. Edits save as you type. A task's checkbox stays clickable while editing. Delete is a destructive button. Escape closes the editor. There is no title count, item counter, move control, or Save button.

## Verification and boundaries

Review capture (empty and filled), the List tab (populated, empty virtual, and inline Add in dark and light themes), item edit, Markdown mode, and toolbar hint in native visual scenes. Exercise storage conflicts, external edits, undo, keyboard input, and same-file saves in tests. Run `scripts/health` and native visual tests, then install with `project install`.

Due dates, priorities, ownership, background task runners, sync, and search across workspaces are outside this version. The mock demonstrates the interaction; native behavior and this spec define the current app.
