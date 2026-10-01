# Workspace worklist

The [single interactive mock](../../visual-review/workspace-queue.html) shows the title-and-bullets worklist in Xenon's light theme. The interaction uses two quiet toolbar actions, a quick capture popover, and a closable worklist tab. The item format below is implemented.

## Experience

Each workspace has one worklist at `<workspace-root>/.xenon/worklist.md`. Nothing is permanently visible or automatically opened. The toolbar's **Add to Worklist** button (`⌘⇧K`) opens quick capture without opening a tab. **Open Worklist** (`⌘⌥K`) opens or focuses the same ordinary, closable editor tab, including one in another pane. Closing the tab keeps the file. A previously open tab may return through normal session restoration.

The two icon buttons sit together in the toolbar's right group after a divider. Their pointer and keyboard-focus hints show the command name and registered chord; icons without shortcuts show a name only. The command catalog drives labels and chords for the toolbar, command palette, and keyboard help. A toolbar action leaves the editor or terminal focused where appropriate. With no workspace, the worklist actions explain that a workspace is needed.

Toolbar capture hosts the same item editor as the worklist tab (title, points, and task checkbox), with no header, hints, or Save button. The title placeholder names the workspace. Title receives focus each time capture opens. Enter adds a point and Tab indents. The item saves as you type once it has a valid title: the first save appends it, later saves rewrite it in place. Escape, ⌘Enter, or outside click closes the popover and restores the previous focus. A fully saved item starts a fresh capture next time and shows a toast offering Undo (⌘Z) for four seconds; unsaved text (no title yet, or a failed save) stays for that workspace during this app session. ⌘⌫ deletes the item and closes. A failure explains the error inline. Capture stays bound to the workspace where it began. When that workspace's worklist tab is open, capture writes through the tab's buffer so the file has one writer; if the tab has unsaved Markdown changes, the user must resolve them before capture writes.

The worklist tab opens in **List** presentation by default. It reads the Markdown file directly and shows top-level checkbox tasks, bullet notes, and existing standalone paragraph notes. **Add an item** opens the same editor as editing. An empty list shows the design system's all-clear state instead of the Add button: a large check that draws itself, "All clear.", and two key chips (↵ add an item, which is also clickable, and ⌘⇧K from anywhere). Return on the empty list opens that editor. Close leaves a saved item in the list. Delete removes it. There is no Save, Cancel, or list Undo button. The list supports checkbox completion, selected-row keyboard navigation (↑/↓, Space, Enter), item editing, and delete. Item editing is the outline; changes save as you type. Cmd-Z undoes typing in the focused field, and Cmd-Shift-Z puts it back. Escape or Close leaves the editor. ⌘⌫ or Delete removes the item. There is no Task/Note choice and no move control. A complex task that the list cannot safely rewrite remains visible; use **Markdown** for it. Markdown mode is the normal editor over the same buffer, with ordinary selection, find, Vim editing, dirty state, and Save. Switching back to List requires saving or undoing Markdown edits first. There is no duplicate task store or separate worklist window. Freeform notes can be written in Markdown mode or by an agent editing the file.

## Sections

Sections are plain Markdown headings; there is no other storage. The first `#` heading is the document title (`# Worklist`), not a section, and the list does not show it. Any later `#` or `##` heading starts a section (`##` preferred). `###` and deeper are ordinary content. Items above the first section show at the top without a header. A file with no sections looks exactly as before.

```md
# Worklist

- Reply to the maintainers   <- unsectioned, shown at the top

## Now
- [ ] Fix focus after closing a split

## Next
```

**List.** Each section has a header row: fold chevron, small-caps name, open-task count ("2 open", "all done" when only done tasks or notes remain, nothing when empty), and a quiet **+ Add** shown on hover or selection. Folding is session view state, never written to the file; a folded section says "N items hidden". An empty section shows "Empty — ↵ to add". ↑/↓ walk headers and items together. On a header, ←/→ fold and unfold, Space toggles, ↵ adds an item that lands at the end of that section, ⌘↵ renames inline (↵ commits, Escape cancels; a blank name deletes the section only when it is empty, otherwise an inline message says to use Markdown), and ⇧↵ starts a new section after the selected row's section and names it. Item keys are unchanged. **+ Add an item** files into the selected row's section (the top when nothing is selected); **+ Section** matches ⇧↵. Clicking a header folds it. Renaming or creating a section edits only its heading line; other text is untouched, and line endings are preserved.

**Capture.** When the file has sections, quick capture shows "Into ‹ Section ›" under the editor. ⌥↑/⌥↓ (or the ‹ › buttons) cycle the options: the sections in file order, preceded by the top when unsectioned items exist. The default is the last section captured into for that workspace during this app session, otherwise the first option. The item is appended at the end of the chosen section, before the next heading, without reformatting anything else. Once the first save lands the chip locks. With no sections the chip is absent and capture appends at the end of the file.

**No move control.** Existing items cannot be moved or reordered between sections from the app (no ⌥↑/⌥↓ item moving, no "Move to…" picker). Moving stays a Markdown-mode edit.

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

Inline edit and quick capture focus a one-line Title field and expose an optional Details field that accepts one point per line, generating the indented Markdown bullets. Task is the default; changing to Note is one click. Saving requires a title. The 80-character title limit is soft: longer titles save. The editor stays quiet until 12 characters remain, then counts down; past 80 the overflow text is tinted amber and a note says long titles wrap in the list. The count uses user-perceived characters (graphemes). Editing a bullet must leave the title in place and preserve unrelated Markdown. Nested subtasks and complex Markdown blocks still require Markdown mode.

The Xenon-installed skill describes this exact file convention and tells agents to create or update items with concise titles and bullet details, preserving unrelated entries. It also clarifies that the worklist is context, not permission to execute a task.

## Proposed nested bullets and typography

Keep each bullet to at most 100 visible user-perceived characters. When an idea needs more room, use a short parent bullet with two-space-indented child bullets. Split by meaning rather than wrapping a sentence at the limit. The agent skill uses this file convention now; the native List view still flattens nested details, so nested items require Markdown mode until the list renderer and editor support them.

The [single mock](../../visual-review/workspace-queue.html) shows nested bullets in List, inline edit, quick capture, and Markdown states. Its typography specimen shows the approved roles for screen title, section title, body, list primary, supporting text, control label, and code. The native design-system typography API now owns their font family, size, weight, line height, and theme color.

The [single Snazzy editor mock](../../visual-review/workspace-queue.html) shows new-in-tab, quick-capture, and edit-existing contexts. The title is the first row in the canvas. Enter or Down from it enters the first point, creating that point if needed; Up from the first point returns to the title. The editor accepts plain text and serializes bullets to Markdown without typed markers or a Task/Note choice. Tab indents the focused point at any caret position. Backspace at its start outdents it, or merges a top-level point into the previous point. Existing note entries remain readable. The native editor is this outline. Edits save as you type. A task's checkbox stays clickable while editing. Delete is a destructive button, also reached with ⌘⌫. Escape closes the editor. There is no item counter, move control, or Save button; the title count appears only near the soft limit.

## Verification and boundaries

Review capture (empty, filled, and with the section chip), the List tab (populated, sectioned in dark and light, a folded section, inline section rename, empty virtual, and inline Add in dark and light themes), item edit, Markdown mode, and toolbar hint in native visual scenes. Exercise storage conflicts, external edits, undo, keyboard input, and same-file saves in tests. Run `scripts/health` and native visual tests, then install with `project install`.

Due dates, priorities, ownership, background task runners, sync, and search across workspaces are outside this version. The mock demonstrates the interaction; native behavior and this spec define the current app.
