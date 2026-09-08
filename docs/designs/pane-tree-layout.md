# Tech design: pane-tree content layout

**Status:** draft for implementation  
**Product source:** `PRODUCT.md` (product model)
**Repo:** xenon  
**Date:** 2026-07-18

---

## Stage 0: Codebase context

### Existing entities

| Entity | Path | Fields / role |
|--------|------|----------------|
| `Layout` | `crates/xenon_core/src/session.rs` | `terminal_visible`, `editor_visible`, `sidebar_visible`, `sidebar_width`, `terminal_width` |
| `SessionState` | same | `layout`, `editors: Vec<OpenEditor>`, `active_editor`, `terminal: TerminalState` |
| `OpenEditor` | same | `path`, `cursor`, `scroll_top` |
| `TerminalState` | same | `cwd` only |
| `TerminalStack` | `crates/xenon_ui/src/app.rs` | `tabs: Vec<Entity<TerminalView>>`, `active` |
| `EditorStack` | same | `tabs: Vec<EditorTab>`, `active` |
| `EditorTab` | same | `path`, `name`, `view` |
| `FocusPane` | `app/deferred.rs` | `Terminal \| Editor \| Browser` (fixed slots) |
| `ResizeEdge` | `resize.rs` | `Sidebar \| Terminal` only |
| `XenonApp` | `app.rs` | parallel `terminals` + `editors` maps; collapse flags; widths |

### Persistence today

- Path: `~/.xenon/sessions/<workspace-uuid>.json`
- Shape: full `SessionState` (serde defaults, unknown fields ignored)
- **UI only reads/writes `layout`** on activate / toggle / resize finish
- Multi terminal tabs: **runtime only**; activate spawns **one** PTY at workspace root if stack missing
- Editors: schema present, **not restored** across restart
- Atomic write: temp + rename; corrupt → `*.corrupt`

### Runtime behavior today

- Main content = fixed horizontal **terminal column | editor column** (`app/render.rs`)
- Separate tab strips per column (`tabs/mod.rs`)
- Last editor tab → `editor_collapsed = true`
- Last terminal tab → empty terminal panel (“⌘N”), panel stays open
- Dirty editor close: Save / Don't Save / Cancel (`dirty_close.rs`)
- No content-tab reorder, no tab DnD (only sidebar workspace list DnD)
- No H/V content split, no unsplit, no pane tree
- Toolbar: Sidebar / Terminal / Editor toggles (`toolbar.rs`)
- Keys: ⌘J terminal, ⌘⇧E editor, ⌘1/2/3 focus slots, ⌃\` cycle, ⌘N new term, ⌘W close focused tab

### Similar patterns to reuse

| Pattern | Where |
|---------|--------|
| Snap-close + residual reopen | `resize.rs`, `panels.rs` |
| Empty teaching UI | `empty_state.rs`, terminal empty body |
| Tab chip strip + context menu | `tabs/mod.rs`, `tabs/menu.rs` |
| Dirty close | `dirty_close.rs` |
| Overlay focus restore | `DeferredUi.pending_focus` |

### Gap (size of haul)

Two typed columns + two stacks → **one tree of mixed leaf panes**. Touches core session types, store migration, app maps, render, resize, focus, keys, toolbar, tab chrome.

---

## Scope

**Problem:** Content layout is a fixed terminal|editor split, but the job is agent-first terminals with optional editors as tabs or on-demand splits.

**In scope (v1):**

1. Domain model: binary **pane tree** of **leaf panes**, each a **mixed tab stack** (terminal and editor tabs in one strip).
2. Session schema + load/save migration from current `Layout` + `editors`.
3. Runtime: replace `TerminalStack`/`EditorStack` dual maps with per-workspace pane tree + live surface entities.
4. Open file → tab in **focused leaf** (no auto-split).
5. New terminal → tab in focused leaf (or sole leaf / empty bootstrap).
6. Split H / V on demand.
7. Split triggers: **toolbar**, **tab drag-and-drop**, **keyboard** (all three required).
8. DnD: drag tab only; drop **center** = move into pane; drop **edge** (L/R/T/B) = split, tab in new sibling.
9. Close cascade: last tab in a leaf with sibling → **unsplit**, unless the leaf is an intentional reserved empty pane; last surface overall → **content empty state**.
10. Pane splitter drag-resize; persist ratios.
11. Sidebar stays optional chrome (unchanged product role).
12. Focus model: focused **leaf id** (+ browser as non-tree focus); cycle leaves + browser.
13. Dirty guards on close tab / unsplit path that would drop dirty editors.
14. Unit tests in `xenon_core` for tree ops (split, move, unsplit, migrate).
15. Dogfood install at end of haul.

**Out of scope (deferred):**

| Item | Why |
|------|-----|
| Multi-terminal PTY restore across app restart | PTYs are live processes; v1 may record tab *placeholders* or skip (see Open questions) |
| Freeform multi-row grids / arbitrary graph | Rule of 7; binary tree is enough |
| Nest depth UI chrome / “maximize pane” | Not needed for agent-first dogfood |
| Accidental empty panes | Still rejected; intentional reserved empty panes are persisted layout slots |
| Mode switcher (Agent vs Inspect) | Layout is enough |
| “Open to the side” from finder as separate product action | Split then open, or add later as thin wrapper |
| Tab reorder within pane via DnD | Nice; not required for split model (optional stretch if cheap) |
| Join/unsplit toolbar button beyond close-last-tab | Unsplit is automatic |
| Replacing sidebar snap-close model | Orthogonal; keep |
| Restoring editor scroll/cursor from session for all tabs | Optional in v1 if cheap; not blocking |
| Full VS Code grid features | Anti-IDE |

**Done means:**

- Default session: one leaf, terminal tabs; open file becomes another tab in that leaf.
- User can split right/down via toolbar button, keybinding, and tab edge-drop; reverse by closing last tab in a half.
- Fully empty content area shows the existing empty state with keyboard path (⌘N / open file).
- Empty panes reuse that same existing empty state. A pane-level × appears only when the pane is empty and the tree has more than one leaf; it removes that exact pane.
- Old session JSON loads without data loss of layout intent (see migration).
- No fixed terminal|editor columns remain as the product model in code paths for the main content area.
- `cargo test` (core + store) green; `project install` dogfoodable.

**Open questions (resolved in this doc where possible):**

| Q | Resolution |
|---|------------|
| Max nest depth? | Soft cap **3** splits from root (max 4 leaves). Further split is no-op + optional status; avoid layout hell. |
| Default split ratio? | **0.5** primary:secondary along axis. |
| Persist terminals? | **v1: do not restore live PTYs.** On load: rebuild tree structure + editor tabs; for terminal slots in tree, spawn **one** fresh terminal in each leaf that had only terminals, or a single terminal in empty leaves. Document clearly. (Option B below if we want richer restore later.) |
| Toggle Terminal / Editor toolbar keys? | Replace with **Split Right / Split Down** (+ keep Sidebar). ⌘J becomes “focus last terminal tab or new terminal in focused leaf” (see contracts). ⌘⇧E becomes “focus last editor or open nothing special” — prefer remap: see Stage 2. |
| One haul vs phase? | **Two PR slices inside one haul** (see Implementation plan): (A) mixed single-pane tabs + session rewrite; (B) tree split/DnD/unsplit. Both required for “done.” |

---

## UX

### Entry points

| Entry | Behavior |
|-------|----------|
| ⌘N / New Terminal toolbar | New terminal tab in **focused leaf**; if content empty, create sole leaf + tab |
| Open file (cmd-p, tree, ⌘O, IDE bridge, path click) | Editor tab in focused leaf; dedupe by path **within workspace** (focus existing wherever it is, optionally move focus to that leaf) |
| Toolbar Split Right / Split Down | Split **focused** leaf; new sibling starts empty **or** with a new terminal (see decision below) |
| Tab drag to pane edge | Split target pane; dragged tab moves into new sibling |
| Tab drag to pane center | Move tab into that pane’s stack; activate it |
| Close tab (⌘W / chip / menu) | Close; dirty guard for editors; then unsplit/empty cascade |
| ⌘B | Sidebar toggle (unchanged) |

**Normal split creates:** new sibling leaf with **no tabs** is forbidden by invariant. The separate reserve-pane action is the intentional exception and marks the empty leaf as reserved. Therefore normal split must either:

- **(Recommended)** Move the active tab into the new sibling and leave the rest in the original, **or**
- Duplicate nothing; require a tab to drag (toolbar split moves **active** tab to new pane)

**Decision:** Toolbar / keyboard split = **move active tab into the new sibling** (VS Code “split” often clones editors; we do **not** clone terminals). If the leaf has only one tab, result is two leaves: one empty-path forbidden — so if single tab, **split is still allowed**: original leaf gets a **new terminal** and the moved tab goes to the sibling *or* simpler: **split with single tab creates sibling that receives that tab and original gets new terminal**. Cleanest agent-first:

> **Split always moves the active tab to the new sibling. If the source leaf would become empty, spawn a new terminal there.**

That never leaves an empty leaf and keeps an agent shell on the “old” side when you peel a file off.

### Primary flows

**A. Agent-only day**

1. Open workspace → one leaf, one terminal (as today).
2. ⌘N more agents as tabs in same leaf.
3. Open file → another tab same leaf; ⌘W closes file; still one pane.

**B. File beside agent**

1. Focus terminal tab; open file (now same pane).
2. Split Right (toolbar or ⌘\ or edge-drag file tab).
3. File tab moves right; left keeps/spawns terminal.
4. Resize splitter.
5. Close file tab → unsplit → single leaf again.

**C. Two files stacked**

1. Two editor tabs; focus one; Split Down.
2. Active editor moves to lower pane.

**D. Empty**

1. Close last tab → content empty state: “⌘N new terminal · open a file”.
2. No splitter chrome.

### States

| State | UI |
|-------|-----|
| Empty content | Existing centered empty state; no tab strip; no splitters |
| Reserved empty pane | Same existing empty state; pane-level × only when another leaf remains |
| Single leaf + tabs | One tab strip (mixed icons/labels); body = active surface |
| Multi-leaf | Recursive split UI; each leaf has own strip; focused leaf focus ring |
| Drag tab | Ghost chip; drop zones: center highlight + 4 edge bands |
| Dirty close | Existing modal; cancel aborts close and unsplit |
| At nest cap | Split command no-ops (no crash); optional brief no toast required v1 |

### Visual (ASCII)

```
+-- toolbar: [sidebar] [split →] [split ↓]  breadcrumb  ... --------+
| sidebar |  leaf A tabs:  term1 | file.rs* | term2                  |
|         |  +--------------------------------------------------+   |
|  ws     |  | active surface                                   |   |
|  files  |  +--------------------------------------------------+   |
|         |======= splitter =====================================   |
|         |  leaf B tabs:  other.rs                                 |
|         |  +--------------------------------------------------+   |
|         |  | editor                                           |   |
+---------+--+--------------------------------------------------+---+
```

Empty:

```
+-- toolbar --------------------------------------------------------+
| sidebar |     No open surfaces                                    |
|         |     ⌘N terminal · open a file                           |
+---------+---------------------------------------------------------+
```

### Keybindings (v1)

| Action | Binding | Notes |
|--------|---------|--------|
| Split Right | `⌘\` | Common editor convention |
| Split Down | `⌘⇧\` | Pair with above |
| New Terminal | `⌘N` | Focused leaf |
| Close focused tab | `⌘W` | Unchanged semantics, new cascade |
| Next / Prev tab | existing | **Within focused leaf** |
| Focus next leaf | `⌃\`` | Cycle leaves then Browser if open |
| Focus terminal slot (legacy ⌘1) | `⌘1` | Focus most-recent terminal tab in tree (or focused leaf’s terminal) |
| Focus editor (legacy ⌘2) | `⌘2` | Most-recent editor tab |
| Focus browser | `⌘3` / `⌘E` | Unchanged |
| Toggle sidebar | `⌘B` | Unchanged |
| **Remove** Toggle Terminal panel | was `⌘J` | Rebind `⌘J` → focus/create terminal in focused leaf (not column toggle) |
| **Remove** Toggle Editor panel | was `⌘⇧E` | Drop or rebind to “focus last editor”; no column toggle |

Toolbar drops Terminal/Editor visibility toggles; adds Split Right / Split Down. Sidebar toggle remains.

### Keyboard-first (hard rule)

Every toolbar and DnD path has a key. DnD is additive.

---

## Domain model

### New entities (`xenon_core`)

```text
PaneId        — opaque id (uuid or monotonic u64 per session; stable in JSON as string/u64)
SplitAxis     — Horizontal | Vertical   // Horizontal = side-by-side (left|right)
// ratio: fraction of space for first child, 0.15..=0.85 clamped

SurfaceKind   — Terminal | Editor

TabId         — opaque id for a tab slot in a leaf (not the PTY/entity)

EditorTabState — path: PathBuf, cursor: Point, scroll_top: u32
TerminalTabState — cwd: PathBuf   // restore hint only; not a live PTY

TabState —
  id: TabId
  kind-specific: Editor(EditorTabState) | Terminal(TerminalTabState)

LeafPane —
  id: PaneId
  tabs: Vec<TabState>        // ordered strip; empty only for a reserved pane
  active: usize              // index into tabs; valid if !tabs.is_empty()
  parked: bool               // intentional empty layout slot

PaneNode —
  Leaf(LeafPane)
  | Split { axis: SplitAxis, ratio: f32, first: Box<PaneNode>, second: Box<PaneNode> }

ContentLayout —
  root: Option<PaneNode>     // None = empty content state
  focused: Option<PaneId>    // must point at a leaf if root Some

SessionState (rewritten) —
  sidebar_visible: bool
  sidebar_width: f32
  content: ContentLayout
  // drop: terminal_visible, editor_visible, terminal_width, flat editors, single terminal
```

### Invariants

1. `root.is_none()` ↔ empty content; `focused` is `None`.
2. If `root` is `Some`, every leaf has tabs after normal split/move/close operations. An empty leaf is allowed only when it is explicitly marked `parked` (reserved).
3. `focused` is always a leaf id present in the tree when `root` is `Some`.
4. `active < tabs.len()` for every non-empty leaf.
5. `ratio` clamped to `[0.15, 0.85]`.
6. Nest depth (split edges on path root→leaf) ≤ **3**.
7. Editor paths unique per workspace (one tab instance); open existing focuses its leaf.
8. No unmarked leaf exists with zero tabs after ops. Reserved empty leaves are intentional and removable when the tree has more than one leaf.

### Modified entities

| Old | New |
|-----|-----|
| `Layout` | Split into sidebar fields + `ContentLayout`; delete term/editor visibility pair |
| `SessionState.editors` | Folded into leaf `TabState::Editor` |
| `SessionState.terminal` | Folded into leaf `TabState::Terminal` hints |
| `TerminalStack` / `EditorStack` (UI) | Replaced by live tree: `HashMap<WorkspaceId, LiveContent>` |

### Live runtime model (`xenon_ui`)

```text
LiveTab —
  id: TabId
  Terminal { view: Entity<TerminalView> }
  | Editor { path, name, view: Entity<EditorView> }

LiveLeaf —
  id: PaneId
  tabs: Vec<LiveTab>
  active: usize

LiveNode — Leaf(LiveLeaf) | Split { axis, ratio, first, second }

LiveContent —
  root: Option<LiveNode>
  focused: Option<PaneId>
```

Pure tree surgery lives in `xenon_core` (or `xenon_ui` pure module if GPUI entities block purity): functions take `ContentLayout` + command → new `ContentLayout` or error. UI layer maps TabId ↔ Entity.

### State transitions

| Trigger | From → To |
|---------|-----------|
| `open_terminal` | empty → single leaf with term; or append tab to focused leaf |
| `open_editor(path)` | empty → single leaf with editor; or focus existing; or append to focused leaf |
| `split(axis)` | focused leaf → Split(source_after_move, new_leaf_with_moved_tab); may spawn term in source |
| `move_tab(tab, target_leaf)` | remove from source (unsplit if empty); append to target; focus target |
| `split_from_dnd(tab, target, edge)` | move tab + insert split on target |
| `close_tab` | remove tab; if leaf empty and sibling → unsplit; if last leaf empty → `root = None` |
| `set_ratio` | update split node ratio |
| `focus_leaf` | set `focused` |
| `activate_tab` | set leaf active index |

### Pure ops API (core)

```text
fn migrate_session_v1(old: OldSessionState) -> SessionState
fn open_terminal(layout, cwd) -> ContentLayout
fn open_editor(layout, path, cursor, scroll) -> ContentLayout
fn split(layout, axis) -> Result<ContentLayout, LayoutError>
fn move_tab(layout, tab, dest_leaf) -> Result<ContentLayout, LayoutError>
fn drop_tab_on_edge(layout, tab, target_leaf, edge) -> Result<ContentLayout, LayoutError>
fn close_tab(layout, tab) -> Result<ContentLayout, LayoutError>  // no dirty; dirty is UI
fn focus_leaf / activate_tab / set_ratio / clamp_tree
fn leaves(layout) -> Vec<PaneId>
fn find_tab(layout, pred) -> Option<(PaneId, TabId)>
fn nest_depth(layout, leaf) -> u32
```

`LayoutError`: `NoFocus`, `AtNestCap`, `UnknownId`, `EmptyMove`.

---

## Contracts

### Module boundaries

| Layer | Responsibility |
|-------|----------------|
| `xenon_core` | Pure tree + session types + migrate + ops + unit tests |
| `xenon_store` | Load/save `SessionState`; no tree logic |
| `xenon_ui` | Live entities, render, DnD, keys, dirty, spawn PTY/editor |

### New UI commands / actions

| Action | Input | Effect |
|--------|--------|--------|
| `SplitRight` | — | `split(Horizontal)` on focused leaf |
| `SplitDown` | — | `split(Vertical)` on focused leaf |
| `NewTerminal` | — | open terminal in focused leaf / bootstrap |
| `CloseFocusedTab` | — | dirty guard then `close_tab` |
| `NextTab` / `PrevTab` | — | within focused leaf |
| `FocusNextPane` | — | next leaf in tree order, then Browser |
| `FocusTerminal` / `FocusEditor` | — | most recent of kind in tree |
| `ToggleSidebar` | — | unchanged |
| **Removed** | `ToggleTerminal`, `ToggleEditor` as panel visibility | replaced |

### Modified behaviors

| Op | Change |
|----|--------|
| `open_editor` | No `editor_collapsed`; always tab-in-leaf |
| `toggle_terminal_panel` | Delete; behavior folded into focus/create terminal |
| `current_layout` / `save_layout` / `apply_layout` | Persist `SessionState.content` + sidebar; snapshot live tree → `ContentLayout` (terminal cwd from view if available) |
| `activate_workspace` | Apply content tree; rebuild live leaves; spawn PTYs for terminal tabs (see restore policy) |
| `render_main` | Recursive render of `LiveNode` |
| Resize | Splitter per `Split` node; sidebar edge unchanged |
| Font zoom | Target focused leaf’s active surface kind |

### Errors / user surface

| Case | Surface |
|------|---------|
| Dirty editor close | Existing dialog |
| Split at nest cap | Silent no-op (v1) |
| Drop on invalid | Cancel drag |
| Close cancelled | No tree mutation |

### Compat

- Session JSON: backward-compatible **load** via migrate; save new shape only.
- No IPC/API for external tools beyond IDE open-file (still path open → `open_editor`).

### Restore policy (contract)

On workspace activate / app start for a session:

1. Load `ContentLayout` (after migrate).
2. For each `TabState::Editor`, open buffer (skip missing files; drop tab; unsplit if needed).
3. For each `TabState::Terminal`, **spawn a new shell** at `cwd` (best-effort; `.` if invalid).  
   **Not** the same process as last run — document in empty/help if needed.
4. If after cleanup tree empty → empty state; optionally ensure one terminal for active workspace dogfood (match today: always have a term when activating workspace).  

**Decision:** Keep today’s “activate workspace ensures at least one terminal” when content would be empty after load.

---

## Schema

### New session JSON (`SessionState`)

```json
{
  "sidebar_visible": true,
  "sidebar_width": 240.0,
  "content": {
    "focused": "1",
    "root": {
      "type": "split",
      "axis": "horizontal",
      "ratio": 0.5,
      "first": {
        "type": "leaf",
        "id": "1",
        "active": 0,
        "tabs": [
          { "id": "t1", "kind": "terminal", "cwd": "." },
          { "id": "t2", "kind": "editor", "path": "src/main.rs", "cursor": { "row": 0, "col": 0 }, "scroll_top": 0 }
        ]
      },
      "second": {
        "type": "leaf",
        "id": "2",
        "active": 0,
        "tabs": [
          { "id": "t3", "kind": "editor", "path": "Cargo.toml", "cursor": { "row": 0, "col": 0 }, "scroll_top": 0 }
        ]
      }
    }
  }
}
```

Empty content:

```json
{
  "sidebar_visible": true,
  "sidebar_width": 240.0,
  "content": { "focused": null, "root": null }
}
```

### Serde representation notes

- `PaneNode` / tabs: internally tagged `type` / `kind` enums.
- Ids: stringified u64 or uuid; stable within file.
- Paths: keep relative-to-workspace convention where possible; migrate absolute → relative on save if under root.

### Migration from old `SessionState`

```text
Old:
  layout.terminal_visible, editor_visible, sidebar_*, terminal_width
  editors[], active_editor, terminal.cwd

Algorithm:
  sidebar_visible/width ← old.layout
  If !terminal_visible && !editor_visible && editors.is_empty():
    content = empty  // rare
  Else if terminal_visible && editor_visible && !editors.is_empty():
    content = Split(
      Horizontal,
      ratio = terminal_width / (terminal_width + assumed_editor) ≈ clamp(terminal_width / 1200, 0.15, 0.85)
        // practical: ratio = clamp(terminal_width / max(terminal_width + 400, 1), 0.15, 0.85)
      first = Leaf { term tab at terminal.cwd },
      second = Leaf { all editors, active_editor }
    )
    focused = first if terminal_visible else second
  Else if editors non-empty (editor only or terminal hidden):
    content = Leaf { all editor tabs }; focused that leaf
  Else:
    content = Leaf { one terminal at cwd }; focused that leaf

  If terminal_visible was false and we created a term-only leaf from “ensure terminal” policy on activate — leave as migrated; activate may still ensure term.
```

Unknown future fields: ignore. After migrate, next save writes new shape only. No dual-write.

### Store

- Still one file per workspace: `sessions/<uuid>.json`
- Tests: old JSON fixtures → migrate → invariants; new JSON round-trip
- `Layout` type: remove or keep as deprecated alias during PR A only; delete by end of haul

---

## Data flow

### Open file (cmd-p)

```text
User confirms path
  → XenonApp::open_editor(path)
  → if existing LiveTab Editor with path: focus its leaf + activate tab; focus view
  → else: core open_editor on ContentLayout snapshot OR mutate LiveContent
  → spawn EditorView; push LiveTab
  → persist session (debounce or immediate; match current save_layout cadence: immediate on structural change)
  → cx.notify(); focus editor view
```

### Split Right (toolbar / ⌘\)

```text
User SplitRight
  → require focused leaf
  → if nest_depth == 3: return
  → identify active tab
  → core/live: create sibling leaf; move active tab; if source empty, spawn TerminalView + tab
  → focused = new sibling (or keep source — prefer **new sibling** so moved surface stays focused)
  → persist; notify; focus moved surface’s view
```

### Tab DnD to edge

```text
Mouse down on tab chip → start drag payload { workspace, tab_id }
Drag over leaf → compute zone: center | Left | Right | Top | Bottom (edge band ~20% or fixed px)
Drop:
  center → move_tab to target leaf (if same leaf: maybe reorder stretch / no-op)
  edge → drop_tab_on_edge (split target along edge; tab into new side)
  → dirty not involved
  → unsplit source if emptied (with spawn-term rule only for toolbar split; for move-all-tabs-away: unsplit, do not spawn)
```

**Close vs move empty source:**

- **Move last tab away** → unsplit source (no spawn).
- **Toolbar split with move active** when others remain → source keeps remaining.
- **Toolbar split with only one tab** → source gets **new terminal** (so user still has a shell).

### Close tab

```text
⌘W / chip
  → if editor dirty: modal; Cancel aborts
  → remove LiveTab; drop entity
  → if leaf empty and not reserved:
       if parent split: replace parent with sibling (unsplit); focus sibling
       else: root = None (empty state)
  → if leaf empty and reserved: keep the leaf as a layout slot
  → persist; notify; focus remaining active surface or nothing
```

### Workspace activate

```text
activate_workspace(id)
  → save previous workspace session snapshot
  → load session / migrate
  → apply sidebar fields
  → build LiveContent from ContentLayout (spawn terms + editors)
  → if live tree empty: ensure one terminal (dogfood)
  → focus focused leaf’s active surface
```

### Async boundaries

- PTY spawn: existing terminal path (sync construct + async PTY inside zed terminal)
- File load: existing editor open
- No new background layout worker

### Trust / validation

- Clamp ratios on load
- Validate tree ids unique; repair focused if missing → first leaf
- Drop editor tabs whose path missing on disk (or keep with error buffer — prefer **open empty/error editor** if current editor does that; else drop)

---

## Implementation plan (not stages of this doc — build order)

### Slice A — Mixed tabs, single pane

1. `xenon_core`: `ContentLayout` types, invariants, migrate from old session, ops for open/close/focus **without** split.
2. Store tests for migrate + round-trip.
3. `xenon_ui`: one leaf per workspace; mixed tab strip; remove dual column render.
4. Remap toolbar/keys for no term/editor panel toggles; empty state.
5. Dogfood: agent tabs + files as tabs only.

### Slice B — Split tree

1. Core split / move / edge-drop / unsplit / nest cap / ratio.
2. Recursive render + splitters.
3. Toolbar split buttons + keybindings.
4. Tab DnD + drop zones.
5. Focus cycle over leaves.
6. Persist full tree; dogfood install.

Do not ship A to “done” without B if the user expects splits; A is intermediate mergeable checkpoint.

### Primary files (expected)

| Area | Files |
|------|--------|
| Core | `crates/xenon_core/src/session.rs` (split modules if >200 lines: `layout_tree.rs`) |
| Tests | `crates/xenon_core/src/tests.rs` or `layout_tree` tests |
| Store | `crates/xenon_store/src/tests.rs` fixtures |
| App state | `crates/xenon_ui/src/app.rs`, `app/sessions.rs`, `app/panels.rs` |
| Term/editor ops | `app/terminals.rs`, `app/editors.rs`, `app/dirty_close.rs` |
| Render | `app/render.rs`, `tabs/mod.rs`, `toolbar.rs`, `resize.rs` |
| Keys/commands | `lib.rs`, `commands.rs`, `app/keyboard.rs`, `app/tree_keys.rs`, `app/navigation.rs` |
| Product | `PRODUCT.md` already updated; this doc is build truth |

---

## Consistency check

| Check | Result |
|-------|--------|
| Entities in contracts defined in model | Yes: ContentLayout, PaneNode, TabState, Live*, actions |
| Schema serves contracts | Session JSON encodes full tree + sidebar; migrate from old |
| Side effects in flows map to contracts | persist session, spawn/drop views, focus |
| Out of scope not required for done | PTY identity restore, tab reorder, open-to-side, grids deferred |
| Open questions closed or flagged | Nest cap, ratio, restore, key remap resolved; tab reorder deferred |
| N/A stages | None — all stages used (desktop app, no HTTP augmentation) |

---

## Appendix: product one-liner

Default surface is a splittable tab grid of terminals; editors are tabs (or panes when split), not a permanent half of the window.

## Appendix: rejected alternatives

| Alternative | Why rejected |
|-------------|--------------|
| Keep fixed columns + “sometimes hide editor” | Fights agent-first; still special-cases editor |
| Modes (Agent / Inspect) | Two concepts for one layout need |
| Clone-on-split for terminals | Meaningless for PTYs; confusing |
| Unmarked empty leaves allowed | Zombie chrome; only explicit reserved empty panes persist |
| Persist live PTY identity | Not available across process death |

---

## Appendix: references

- `PRODUCT.md` — product model
- Exploration: dual stacks in `xenon_ui/src/app.rs`; layout in `xenon_core/src/session.rs`
