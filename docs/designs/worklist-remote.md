# Worklist on the phone: one owner, content-keyed ops

## Problem

The phone service (`xenon_phone`, its own process, no GTK, works with no desktop window)
must read and change `<root>/.xenon/worklist.md`. The logic lived in the GTK crate, and
`xenon_phone` can't depend on it. Constraints:

- **Addressing.** Index ops are valid only against the exact source they were parsed
  from. A phone holds its view for minutes while agents edit the file, so index plus
  whole-file compare-and-swap would fail constantly.
- **Several writers.** Desktop list, capture dialog, phone, and the editor tab write the
  same file. The atomic compare-and-swap write stays the one primitive, with one owner.
- **Invariants.** Existing Markdown is rewritten only for the item being changed; unsafe
  Markdown (open fence or front matter, nested checkboxes, code, tables) is left alone;
  writes are atomic and checked against what was read.

## Usage (caller's view)

```rust
use xenon_worklist::{Op, Place, ItemText, Effect};

let now = xenon_worklist::read(&root)?;                  // Snapshot; missing file = empty
let done = xenon_worklist::apply(&root, Op::SetDone { item, done: true })?;
render(&done.now);                                       // always render `now`
if let Effect::Gone = done.effect { notice("That changed on your computer.") }

// Capture dialog autosave: Add, then Replace on the key the Add returned
let added = apply(&ws, Op::Add { to: place, text })?;
apply(&ws, Op::Replace { item: key, text })?;

// Delete with undo; the phone keeps the last Undo per workspace in memory
if let Effect::Saved { undo, .. } = apply(&ws, Op::Remove { item })?.effect { /* later: revert(&ws, &undo) */ }

let path = xenon_worklist::ensure(&root)?;               // creates "# Worklist" if missing
```

## Shape

New crate `crates/xenon_worklist`. Every module is private; the public surface is
`read`, `apply`, `revert`, `ensure`, `PATH`, `TITLE_LIMIT`, `title_length`, and the
types `Snapshot`, `Group`, `Item`, `ItemKey`, `SectionKey`, `Place`, `ItemText`, `Op`,
`Applied`, `Effect`, `Undo`. No serde: wire types stay in `xenon_phone`.

- **Keys.** `ItemKey` is the first 8 bytes of SHA-256 over the item's trimmed raw
  Markdown (task marker normalized to `- [ ] `) plus how many earlier entries share it.
  Checking an item or editing other items doesn't change its key; editing the item does,
  and then ops on the old key report `Gone`. `SectionKey` is the same over the heading
  title. `Place` is `top` or a section key.
- **`apply`.** Reads, parses, runs the pure `plan(&Doc, &Op)`, then writes with the
  compare-and-swap. On a lost race it re-plans by key, up to three times. `Applied.now`
  is always the file after the call; `Effect` is `Saved { item, undo }`, `Unchanged` or
  `Gone`. Refusals ("Edit this one in Markdown") are errors; staleness never is.
- **Desktop.** The list renders from `Snapshot` and keeps its `i32` menu actions: the
  integer is a row (items) or group (sections) index into the snapshot it rendered, and
  `RowInfo` maps it to `ItemKey` or `Place`. The capture dialog holds an
  `Option<ItemKey>`: first save is `Add`, later saves `Replace`. `Draft`, `DraftEdit`,
  `commit_draft` and `try_update` are gone. The list reloads on file change and when the
  window regains focus, skipping the rebuild when the snapshot is equal.
- **Phone HTTP.** `GET /api/workspaces/{id}/worklist` and `POST` with
  `{"op":"setDone"|"add"|"remove"|"undo", ...}` both return `WorklistInfo` (`groups`,
  optional `notice`, optional `undo` token). Closed workspaces work.
- **Phone undo.** A successful `remove` stores its `Undo` with a token, one per
  workspace, in memory. `undo` reverts it; if the file changed since, the reply is the
  fresh list with "Can't undo — the worklist changed since." The page shows a toast with
  an Undo button after delete, so delete has no confirm.
- **Phone page.** A Worklist button on the terminals view opens `v-worklist`. Every
  response re-renders the whole list; the page re-fetches when it opens and when the
  page becomes visible again (unless an add field has text). No optimistic UI.
- **First add** on a missing file creates `.xenon/worklist.md` with a `# Worklist`
  heading, same as desktop.

## Synthesis decision

One candidate: the existing GTK-free code moved whole. New: content keys (generalized
from `Draft`), the pure `plan`, and the `Applied { now, effect }` result.

## Tradeoffs accepted

- We accept a breaking refactor of the desktop list and capture dialog in exchange for a
  single write path.
- We accept `Gone` for an item edited elsewhere, even a typo fix, in exchange for never
  acting on text the user didn't see.
- We accept `Unchanged` when removing an item that's already missing, in exchange for
  idempotent retries.
- We accept a duplicate on a retried `Add` after a dropped response; the phone disables
  the control while a request is in flight.
- We accept `nth` shifting between exact duplicates; their text is identical.
- We accept 404 for refusals (the existing `reply()`); the page only reads `error`.
- We accept that phone undo is in memory and per workspace, last removal only; a host
  restart or any later write makes it unavailable.
- We accept no live push to the phone; it re-fetches on open, on visible, and after
  every op.
- Phone scope is view, check, add (title only), delete and undo. Details editing, move
  and sections stay out of HTTP; the crate supports them.

## Alternatives considered

- **Indices plus a document revision, 409 on mismatch.** Every caller must carry and
  echo a revision, and the CAS race can't be retried internally. It also fails the main
  phone case: agents append while you're away and every first tap would fail.
- **Copy the ops into `xenon_phone`.** Two writers with diverging rules.
- **Phone asks the desktop over IPC.** Breaks "works with no desktop window open".
- **A side journal merged at read.** Agents and the editor tab read the Markdown
  directly and wouldn't see it.

## Open questions and risks

- Should the phone gain details editing or section moves later? It's a route and a wire
  variant each.
- Does a one-slot undo per workspace feel right if two phones are paired?
- Key derivation must hash exactly the bytes `parse` assigns to an entry, or capture
  autosave loses its item.

## Next implementation step

Shipped; next is exercising it from a phone against a live workspace.
