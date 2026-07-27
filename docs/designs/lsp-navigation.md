# Tech design: LSP navigation + document highlight

**Status:** implemented 2026-07-27 (settled nav UX 2026-07-26)  
**Product source:** `PRODUCT.md` (editor enough to live here; anti-IDE), prior chat  
**Repo:** xenon  
**Date:** 2026-07-24 (rev 2026-07-27)  
**Languages (v1):** Rust (`rust-analyzer`), TypeScript/JavaScript (see § TypeScript server tradeoffs)

---

## Stage 0: Codebase context

### Existing entities

| Entity | Path | Role / gap |
|--------|------|------------|
| `Buffer` | `crates/xenon_editor/src/buffer/mod.rs` | rope, path, cursor (char offset), dirty via version, `set_cursor_position(row, col)` |
| `EditorView` | `crates/xenon_editor/src/view.rs` | build from path only; tree-sitter re-highlight; disk poll; `EditorEvent::SelectionChanged` |
| `Lang` | `crates/xenon_editor/src/highlight/lang.rs` | path → language for **syntax only** (not public, not LSP languageId) |
| Search paint | `element/paint.rs`, `view/layout.rs` | char ranges → monospaced rects; only selection + find matches |
| `open_editor` | `xenon_ui` `app/content_ops.rs` | **path + focus only** — no line/col; focusing existing tab does not jump |
| `NavHistory` | `app/nav_history.rs` | per-workspace stack of **`TabId` only** (⌘[ / ⌘]); its index points at the current entry |
| `AppSettings` | `xenon_store/src/settings.rs` | fonts, themes, vim, window geometry — **no LSP fields** |
| Workspace root | `XenonApp` / `WorkspaceRec.root` | one root per workspace; `workspace_root(id)` |
| Editor mouse | `mouse.rs` + `view/input.rs` | left click / shift extend / multi-click; **no ⌘-click** |
| Positions | `selection::offset_line_col` | 0-based line + **char** column — **not UTF-16** (LSP default) |
| IDE bridge | `xenon_ide` | Claude MCP; `openFile` path-only; `getDiagnostics` stub `[]` |

### Similar features to reference

| Feature | Path | Pattern to reuse |
|---------|------|------------------|
| Find match paint | `find.rs` + layout `search_matches` | range list → rects → quieter fill |
| Terminal path open | `xenon_terminal` paths + ⌘-click | modifier-click navigation |
| Disk conflict banner | `view/disk.rs` | quiet in-editor status strip |
| Selection → IDE | `wire_editor_selection` | editor events out to shell |

### Conventions

- New capabilities as workspace crates under `crates/`; UI wires in `xenon_ui`.
- Settings: optional fields on `AppSettings` with `#[serde(default)]`.
- Keyboard-first: action + binding + Escape/no-op when unavailable.
- Async work via gpui tasks; never block paint/input on network/process I/O.
- Dogfood via `project install` once haul is complete.

### Explicit absences (load-bearing)

1. No language-server client, process, or JSON-RPC.
2. No open-at-position end-to-end.
3. No decoration layer beyond selection + find.
4. No location (jump) stack — only tab history.
5. No buffer change bus for external subscribers (only internal version + selection event).

---

## Scope

**Problem:** The editor cannot jump to symbol definitions or highlight true same-symbol occurrences; those need a language server, not tree-sitter or text search.

**In scope (v1):**

1. Thin LSP client crate (`xenon_lsp`): stdio JSON-RPC, initialize, shutdown, lifecycle.
2. Servers for **Rust** and **TypeScript/JavaScript** (one process per workspace root × language family).
3. Document sync for open editor buffers (`didOpen` / `didChange` / `didClose` / `didSave`).
4. **Go to definition** (and multi-location pick when server returns several).
5. **Document highlight** (same-symbol occurrences under cursor).
6. **Location stack** (jump list): record origin and destination on successful go-to-def; go back / go forward.
7. **Open at position** plumbing (`open_editor` + existing tab + cursor + scroll).
8. **Minimal diagnostics:** receive `textDocument/publishDiagnostics`; underline ranges + gutter marks + next/prev diagnostic; no full problem panel.
9. Entry points: keybind, vim `gd`, ⌘-click on identifier.
10. Quiet failure when binary missing or request fails (status once, no freeze).

**Out of scope (deferred — why):**

| Item | Why deferred |
|------|----------------|
| Completions / signature help | Large UX surface; agents already assist |
| Rename / code actions / format | High risk, needs preview UI |
| Semantic tokens (LSP coloring) | Tree-sitter already colors; separate haul |
| Hover popover | Nice; not required for nav dogfood |
| Find references panel | Nav v1 = definition; refs can follow |
| Workspace symbol / file outline | Separate picker design |
| Full problem panel / diagnostics list UI | v1 paints in-buffer; list is chrome |
| Multi-root single server | One workspace root per Xenon workspace is enough |
| VS Code/Cursor `settings.json` project LSP config | Start PATH + simple override |
| Pulling Zed project/LSP crates | Too coupled; GPL surface; wrong product shape |
| Languages beyond Rust + TS/JS | Prove client first |
| Claude `getDiagnostics` real implementation | Optional follow once local diagnostics exist |
| Incremental text sync only | Full-document `didChange` fine for dogfood sizes; upgrade if lag |

**Done means:**

| # | Criterion |
|---|-----------|
| G1 | In a Rust workspace with `rust-analyzer` on PATH, F12 / `gd` / ⌘-click on a symbol opens the definition (new tab or jump in open tab) at the correct line/col |
| G2 | Same for a TS/JS project with the chosen TS server on PATH |
| G3 | Idle cursor on a symbol shows other occurrences (documentHighlight) with a themeable background, distinct from selection and find |
| G4 | After go-to-def A → B, the stack contains current locations A then B; ⌘[ restores A and ⌘] restores B |
| G5 | ⌘[ / ⌘] are the **unified navigation stack** (see UX); sequential tab cycle stays ⌃⇥ / ⌘⇧[ |
| G6 | Missing server: no crash; one quiet status; nav is a no-op |
| G7 | Diagnostics for the active file show as underline + gutter; next/prev diagnostic moves cursor |
| G8 | Typing stays smooth (async requests, cancel on cursor move / new edit) |
| G9 | Unit tests cover position encoding (UTF-16), jump stack, and JSON-RPC framing; no GUI automation required |
| G10 | `project install` once at end of haul for dogfood |

**Open questions (resolved in this doc unless flagged):**

| Question | Resolution |
|----------|------------|
| Languages | Rust + TypeScript/JS |
| Navigation stack | **⌘[ / ⌘]** — unified stack (editor locations + prior surface visits); not separate ctrl-o |
| Multi-definition | **1 result → jump; 2+ → pick list** |
| TS server binary | Prefer `typescript-language-server`, else `vtsls` — see tradeoffs |
| Server discovery | Option B: PATH defaults + optional settings overrides |
| Diagnostics in v1 | Minimal in-buffer (not a problem panel) |

---

## Server discovery (explained)

“Discovery” means: *when you open `foo.rs` under workspace root `/proj`, how does Xenon decide which binary to spawn, with what args, and whether to spawn at all?*

### Option A — Hardcoded PATH defaults only

| Language | Root markers (any) | Binary (first hit on `PATH`) | languageId examples |
|----------|--------------------|------------------------------|---------------------|
| Rust | `Cargo.toml` | `rust-analyzer` | `rust` |
| TS/JS | `package.json`, `tsconfig.json`, `jsconfig.json` | Resolve per § TypeScript server tradeoffs | `typescript`, `typescriptreact`, `javascript`, `javascriptreact` |

- Spawn when: an editor opens a file whose language maps to a family **and** root markers exist under the workspace root (search upward from file toward workspace root).
- No config UI. Override = rename/symlink on PATH.

**Pros:** Zero config, matches “it just works” for standard installs.  
**Cons:** Custom install paths (e.g. `~/.local/share/nvim/...`) need PATH hacks.

### Option B — PATH defaults + `settings.json` overrides (recommended)

Same defaults as A, plus optional durable fields:

```json
{
  "lsp": {
    "rust": { "command": "/opt/homebrew/bin/rust-analyzer", "args": [] },
    "typescript": { "command": "typescript-language-server", "args": ["--stdio"] }
  }
}
```

- Empty / missing → Option A behavior.
- `command` may be absolute or PATH name.
- No per-project file in v1.

**Pros:** Fixes real “binary not where we expect” without a marketplace. Fits existing `AppSettings` pattern.  
**Cons:** Slight settings surface; still no monorepo multi-version matrix.

### Option C — Project-local config (`.xenon/lsp.json` or VS Code-compatible)

Per-checkout server command, env, init options.

**Pros:** Power-user / multi-version monorepos.  
**Cons:** Another config dialect; agents may invent broken JSON; not needed for dogfood.

### Decision

**v1 = Option B** (PATH defaults + optional settings overrides).  
Defer project-local config until a real monorepo pain appears.

**No spawn when:** language unknown, no root markers, or binary resolution fails → editor works as today (tree-sitter only).

---

## TypeScript server tradeoffs

Both speak LSP over stdio and wrap Microsoft’s TypeScript language service. Xenon does **not** bundle Node or a server; user installs one.

### Candidates

| | **`typescript-language-server`** (sourcegraph / community) | **`vtsls`** (yioneko) |
|--|--|--|
| **What it is** | Thin LSP adapter around `tsserver` | LSP adapter aiming at VS Code TS/JS extension parity |
| **Install** | `npm i -g typescript-language-server typescript` (needs `typescript` nearby) | `npm i -g @vtsls/language-server` (or `vtsls`) |
| **Binary / args** | `typescript-language-server --stdio` | `vtsls --stdio` |
| **Maturity / mindshare** | Older default in Neovim, Helix, many configs; lots of blog/copy-paste | Newer; popular where people want VS Code–like TS behavior |
| **Features beyond nav** | Definition, highlight, diagnostics — solid. Completions/rename/code actions good enough for later | Stronger “VS Code TS” feature surface (auto-imports, some code actions) when we grow past v1 |
| **Resource / complexity** | Usually lighter mental model; fewer special cases | More VS Code protocol surface; slightly more moving parts |
| **Project resolution** | Uses workspace `typescript` / tsconfig; occasional “wrong tsserver version” footguns | Generally better at matching VS Code’s project graph expectations |
| **Risk for Xenon v1** | Well-trodden; fewer surprises for go-to-def + diagnostics | Fine for v1 too; slightly less ubiquitous on random machines |

### What does **not** matter for v1

Completions, inlay hints, organize imports, semantic tokens — both can do more than we will call. **Definition, documentHighlight, publishDiagnostics** are table stakes on either.

### Decision

**Resolve in order:**

1. Settings override `lsp.typescript.command` if set.  
2. Else `typescript-language-server` on PATH → args `["--stdio"]`.  
3. Else `vtsls` on PATH → args `["--stdio"]`.  
4. Else no TS server (quiet status once).

**Why TLS first:** wider install base and docs for “minimal editor + LSP”; v1 only needs the thin slice both provide. Override still allows power users on `vtsls`. Do not auto-install either.

---

## Diagnostics (explained)

### What they are

Language servers push **diagnostics** — structured problems for a document:

- severity: error / warning / info / hint  
- range in the file  
- message + optional code + source (`rustc`, `eslint`, …)

Protocol: server → client `textDocument/publishDiagnostics` (unsolicited after sync/analysis). Once document sync works, **you already pay for the analysis**; ignoring the push throws away free signal.

### Why they pair with nav + highlight

| Capability | Needs sync + server alive? | Extra UI cost |
|------------|----------------------------|---------------|
| documentHighlight | yes | paint ranges (like find) |
| definition | yes | open at position + jump stack |
| diagnostics | yes (same) | underlines + gutter + next/prev |

If a symbol is red because the project does not compile, **seeing why** while jumping around is the same “live here” bar as go-to-def. Anti-IDE does not mean blind editor; it means no harness religion and no chrome bloat.

### What we are *not* building in v1

- Bottom “Problems” panel / tree of all workspace issues  
- Quick-fix / code actions from diagnostics  
- Filtering, severity toggles, export  

### What we *are* building in v1 (minimal)

1. Store last diagnostics per URI for open docs.  
2. Paint error/warning underlines (or background tint) on ranges.  
3. Gutter mark on lines that have diagnostics.  
4. Status: quiet count for active buffer (e.g. strip or existing chrome — keep tiny).  
5. **Next / previous diagnostic** keybinds move cursor to next range in file.  
6. Hover message **optional defer** — if cheap, status bar shows message for diagnostic under cursor; full hover card later.

**Recommendation:** Include minimal diagnostics in the same haul as nav + highlight. Exclude problem panel. If schedule slips, drop next/prev first, keep paint.

---

## UX

### Entry points

| Action | Binding (settled) | Context |
|--------|-------------------|---------|
| Go to definition | `F12`; vim normal `gd` | Editor focused |
| Go to definition | ⌘-click on identifier | Editor |
| Navigate back | **⌘[** | Global (same chords as today) |
| Navigate forward | **⌘]** | Global |
| Sequential tab cycle | ⌃⇥ / ⌘⇧[ (unchanged) | Global |
| Next diagnostic | `F8` | Editor |
| Previous diagnostic | `shift-F8` | Editor |
| Document highlight | automatic on idle cursor (no key) | Editor + server ready |

Palette entries: **Go to Definition**, **Go Back**, **Go Forward**, **Next Diagnostic**, **Previous Diagnostic**.

### ⌘[ / ⌘] = unified navigation stack (settled)

**Product intent:** One pair of chords for “where I was,” covering both agent-shell surface switches and symbol jumps — not two competing histories.

**Replace** today’s tab-only `NavHistory` with a **`NavStack`** of positions. As
in the current implementation, `index` always identifies the location currently
shown; Back decrements it and Forward increments it.

| Entry kind | When pushed | Restore does |
|------------|-------------|----------------|
| `Editor { path, row, col }` | An editor becomes the current surface; immediately before leaving it, refresh its current entry with the latest cursor | `open_editor_at` + focus |
| `Terminal { tab_id }` (or surface id) | A terminal becomes the current surface (same triggers as current tab visit) | Focus that terminal tab |

**Rules (browser-style):**

- Truncate forward branch on new push.  
- Cap ~50.  
- Normal surface visit: refresh the entry being left, truncate the forward branch, append the destination, and make it current. Deduplicate only exact adjacent entries.  
- Go-to-def A → B: snapshot A, open B, then refresh A, truncate the forward branch, append B, and make B current. If B cannot be opened, do not mutate the stack.  
- No push when restore is itself navigating the stack.  
- Prune dead tabs / closed paths.

**What we give up vs pure tab history:** nothing important if every current `nav_visit` also records editor cursor (or terminal tab). Sequential “next tab” remains ⌃⇥ — not ⌘[.

**What we refuse:** separate `ctrl-o` location stack *and* ⌘[ tab stack (two mental models).

### Primary flows

**Go to definition**

1. User places cursor on symbol (or ⌘-clicks).  
2. Client ensures doc synced; sends `textDocument/definition`.  
3. **Exactly one** location: snapshot the origin; `open_editor_at`; after it succeeds, record origin + destination in `NavStack`; reveal line.  
4. **Two or more** locations: show a **pick list** (palette-style: path, line, preview snippet if cheap); on confirm, snapshot the origin, open the target, and after success record origin + destination. Escape cancels — **no** stack mutation.  
5. **Zero** / error: status flash; no stack push; no pick list.

**Document highlight**

1. Cursor settles (debounce ~150–300ms) on a word-class position.  
2. Cancel prior highlight request.  
3. `textDocument/documentHighlight` → ranges with kind (read/write/text).  
4. Paint non-selection backgrounds; clear on move to non-identifier, multi-line selection, or buffer change.  
5. If server missing: no lexical fallback in v1 (avoid two truths); simply no highlights.

**Diagnostics**

1. Server publishes for a URI.  
2. If buffer open, store + notify editor paint.  
3. F8 cycles ranges sorted by start position.

### States

| State | Behavior |
|-------|----------|
| No server / binary missing | Nav no-op; one-time status per workspace language (“rust-analyzer not found”) |
| Server starting | Requests queue or no-op until `initialized`; no spinner required for v1 |
| Server crashed | Log; clear highlights; mark language dead until next open/reopen workspace |
| Loading definition | Optional no UI; must not block keys |
| Success | Jump / paint |
| Edge: definition outside workspace | Still open path if readable; if not, status |
| Edge: unsaved buffer | Sync full text before request so results match buffer |

### Visual

```
[ gutter | line text with ~~~error underline~~~ and ##occurrence## bg ]
         ^ diagnostic mark
```

- Occurrence: softer than selection; distinct from find match (theme keys).  
- Diagnostic underline: severity-colored (error/warning).  
- Paint order (extend existing): **diagnostic underline → search matches → occurrences → selection → cursor → text → gutter marks**.

### Keyboard-first

All flows work without mouse. ⌘-click is additive.

---

## Domain model

### New entities

**`LanguageId`** — LSP string (`"rust"`, `"typescript"`, …). Derived from path (share mapping table with highlight extensions where possible).

**`ServerId`** — `(WorkspaceId, LanguageFamily)` e.g. `Rust` | `TypeScript` (TS+TSX+JS+JSX one server).

**`LspServer`** — child process + JSON-RPC state: `Starting | Running | Failed | Stopped`. Holds initialize result capabilities.

**`LspDocument`** — `uri`, `language_id`, `version: i32` (LSP version; bump on each didChange), last synced text fingerprint optional.

**`EditorLocation`** — `path: PathBuf`, `row: u32`, `col: u32` (editor **char** columns for UI; convert at LSP boundary).

**`NavEntry`** — `Editor(EditorLocation) | Terminal { tab_id: TabId }` (extend later only if new surface kinds need restore).

**`NavStack`** — per-workspace stack: `stack: Vec<NavEntry>`, `index`, cap ~50.
`stack[index]` is always the currently displayed location. Snapshot the origin
before a transition; after the transition succeeds, use that snapshot to refresh
the current entry, append the destination, and advance `index`. **Replaces**
tab-only `NavHistory` behind ⌘[ / ⌘].

**`DefinitionCandidate`** — one pick-list row: `path`, `row`, `col`, optional `label` (e.g. container name from server if present).

**`DocumentHighlight`** — `range: Range<CharOffset>`, `kind: Read | Write | Text`.

**`Diagnostic`** — `range`, `severity`, `message`, `source`, `code?`.

**`DiagnosticSet`** — per URI: diagnostics plus the server-supplied document
version when present, otherwise the current synced version at receipt. Replaces
the whole accepted set on each publish (LSP semantics).

**`LspRegistry` / `LspHost`** (app-owned) — map of servers, documents, diagnostics; spawns/stops with workspace lifetime.

### Modified entities

| Entity | Change |
|--------|--------|
| `EditorView` / layout | Hold `occurrence_ranges`, `diagnostic_ranges` (or query host each prepaint); paint hooks |
| `XenonApp` | Own `LspHost`; wire open/close/edit; upgrade `NavHistory` → `NavStack`; actions; definition pick list |
| `open_editor` | Become fallible `open_editor_at(path, focus, Option<Location>)`; navigation commits history only on success |
| `AppSettings` | Optional nested `lsp: LspSettings` (command overrides) |
| Buffer / view | Emit **edit notifications** (path, version, full text or change) for sync — not only selection |

### Relationships

- One `LspHost` per app (or per workspace map inside host).  
- Many `LspDocument`s per server (all open files of that family under root).  
- `NavStack` per workspace; owns ⌘[ / ⌘] (replaces `NavHistory`).  
- Diagnostics keyed by URI; editor paints by path match.

### State transitions

```
Server: Absent → Starting (spawn) → Running (initialized)
                 ↘ Failed (spawn/init error)
Running → Stopped (workspace close / shutdown)
Running → Failed (process exit) → optional restart on next request (v1: manual reopen workspace)

Document: Closed → Open (didOpen) → Dirty sync (didChange debounced) → Closed (didClose)
Highlight: Idle → Requesting → Painted | Cleared
```

### Invariants

- LSP `Position.character` is **UTF-16** code units; rope UI uses **chars**; conversion only at client boundary.  
- Document `version` strictly increases on didChange.  
- `NavStack.stack[index]` represents the current surface; a successful A → B
  transition leaves adjacent entries A and B with `index` on B.  
- Navigation stack mutation occurs only after the destination is validated and
  the transition can be applied.  
- Never block input thread on server I/O.  
- No server ⇒ editor fully usable (syntax, find, vim).
- A diagnostic publication with a supplied version older than the latest synced
  version is discarded. Any local edit immediately clears painted diagnostics
  until another publication is accepted.

---

## Contracts

### New crate: `xenon_lsp`

**Purpose:** Process lifecycle + JSON-RPC + typed requests used by v1. No GPUI.
The crate owns its process-reader/writer workers and exposes runtime-neutral
futures and event streams; `xenon_ui` is responsible for adapting those futures
to GPUI tasks and applying results on the main thread.

**Public surface (sketch):**

```text
LspHost
  open_workspace(root: PathBuf) / close_workspace(root)
  open_doc(path, language_id, text) / close_doc(path) / did_change(path, version, text) / did_save(path)
  definition(path, pos: LspPosition) -> BoxFuture<Result<Vec<Location>, LspError>>
  document_highlight(path, pos) -> BoxFuture<Result<Vec<DocumentHighlight>, LspError>>
  diagnostics(path) -> Vec<Diagnostic>   // last published snapshot
  subscribe() -> EventReceiver           // DiagnosticsChanged, ServerStateChanged

LspPosition { line: u32, character: u32 }  // UTF-16
Location { path, line, character_utf16 }   // convert in UI layer to editor col
```

**Transport:** stdin/stdout, Content-Length framing, `serde_json` + `lsp-types`.
The reader distinguishes responses, notifications, and server→client requests.
Every server request receives a JSON-RPC result or error response; unsupported
methods receive `MethodNotFound` rather than being ignored.

**Errors:** `NotAvailable` (no server), `Timeout`, `Rpc`, `Io`, `NoResult`.

**Auth:** N/A (local process).

### Modified: `open_editor`

```text
open_editor_at(path, focus, at: Option<(row, col)>) -> Result<(), OpenEditorError>
```

- Existing tab: activate + `set_cursor_position` + ensure visible.  
- New tab: build then set position.  
- `open_editor(path, focus)` = `open_editor_at(path, focus, None)`.
- Success means the target tab is active and its requested cursor position has
  been applied. Navigation callers mutate `NavStack` only after this result.

### Modified: editor → shell events

```text
EditorEvent::BufferEdited { path, version, text }  // or callback registration
EditorEvent::CursorSettled { path, row, col }     // debounced, for highlight
// keep SelectionChanged for IDE bridge
```

### New actions (names illustrative)

| Action | Crate |
|--------|-------|
| `GoToDefinition` | editor or ui |
| `LocationBack` / `LocationForward` | ui |
| `NextDiagnostic` / `PrevDiagnostic` | editor or ui |

### LSP methods used (v1)

| Method | Direction | Use |
|--------|-----------|-----|
| `initialize` / `initialized` / `shutdown` / `exit` | both | lifecycle |
| `textDocument/didOpen` `didChange` `didClose` `didSave` | client→server | sync |
| `textDocument/definition` | request | nav |
| `textDocument/documentHighlight` | request | occurrences |
| `textDocument/publishDiagnostics` | server→client | diagnostics |
| `workspace/configuration` | server→client request | return empty/default sections requested by server |
| `window/workDoneProgress/create` | server→client request | acknowledge; progress notifications may be ignored |
| `client/registerCapability` / `client/unregisterCapability` | server→client request | acknowledge only registrations the client can honor; otherwise return an error |
| `workspace/applyEdit` | server→client request | return `applied: false` (editing actions are out of scope) |
| Other server requests | server→client request | JSON-RPC `MethodNotFound` |
| `workspace/didChangeConfiguration` | optional client notification | skip while defaults/settings are fixed for a running server |

**Capability negotiation:**

- Client initialize capabilities advertise UTF-16 position support and the
  response shapes Xenon can decode for definition/documentHighlight. Do not
  claim dynamic registration unless the request handler will honor it.
- Store the server's initialize result, including `textDocumentSync`,
  `definitionProvider`, `documentHighlightProvider`, and selected
  `positionEncoding`.
- Gate each request and sync notification on the corresponding server
  capability. No capability means a quiet `NotAvailable`, not a speculative RPC.
- v1 implements full-document sync even if a server also supports incremental;
  if the server declares no compatible sync mode, disable that server.

### Position encoding

- Advertise `general.positionEncodings: ["utf-16"]`; omitted server
  `positionEncoding` means UTF-16 by protocol default. Reject/disable a server
  that selects an encoding Xenon did not advertise.  
- Store the negotiated encoding per server. v1 conversion helpers are
  `char_col_to_utf16` and `utf16_to_char_col` per line.  
- Unit tests: emoji / non-BMP on a line.

### TS / Rust init options

- **rust-analyzer:** empty or minimal (`cargo`/`procMacro` defaults).  
- **TS:** resolved binary with `--stdio`; root = workspace root.  
- Do not ship custom tsserver path logic beyond `rootUri`.

---

## Schema

**Persistence:** only optional settings. No session DB for jump stack (in-memory). Diagnostics not persisted.

### `AppSettings` addition

```json
{
  "lsp": {
    "enabled": true,
    "rust": { "command": null, "args": [] },
    "typescript": { "command": null, "args": ["--stdio"] }
  }
}
```

- `command: null` → default binary name on PATH.  
- `enabled: false` → never spawn (escape hatch).  
- Serde defaults: all optional; missing `lsp` key = enabled + defaults.

**Migration:** none; new fields default.

**Indexes / tables:** N/A.

---

## Data flow

### Path A — Go to definition

```
Keybind / ⌘-click / gd
  → EditorView resolves cursor (row, col_char)
  → XenonApp / LspHost: ensure server for (root, family)
  → ensure didOpen/didChange up to date (await in-flight debounce flush)
  → convert to UTF-16 Position
  → textDocument/definition (runtime-neutral future, polled by a GPUI task)
  → on result:
       0 locations → status; stop
       1 location  → snapshot origin; open_editor_at;
                     on success refresh origin + append destination; notify
       2+          → open definition pick list (no push yet)
                     on pick → snapshot origin; open_editor_at;
                               on success refresh origin + append destination
                     on Esc  → no stack mutation
  → on NotAvailable: status once
```

**Async boundary:** all RPC on background; apply results on main only if buffer version / request id still current.

**Cancel:** new definition request supersedes previous (ignore stale id). Pick list closes on Esc without applying.

### Path B — Document highlight

```
Cursor move / edit
  → debounce
  → if not identifier-ish, clear paints
  → documentHighlight request (cancel prior)
  → layout stores ranges → paint under selection
```

### Path C — Diagnostics

```
Server publishDiagnostics(uri, diags)
  → if supplied version < latest synced version: discard
  → otherwise LspHost replaces DiagnosticSet for uri
    (versionless publication is tagged with current synced version)
  → if EditorView open for path, notify
  → layout underlines + gutter
F8 → sort ranges → move cursor to next → ensure visible

Buffer apply_edit
  → clear painted diagnostics immediately
  → wait for a newly accepted publication
```

### Path D — Edit sync

```
Buffer apply_edit
  → bump local version
  → clear diagnostics and stale highlight/definition results for prior version
  → debounce 50–150ms
  → didChange full text + version
```

When a server omits `publishDiagnostics.version`, the client cannot prove which
analysis generation produced it. v1 accepts it as the newest server snapshot,
tags it with the current synced version, and clears it again on the next local
edit. This is the conservative behavior available without pull diagnostics.

**Trust:** all paths are local filesystem + local process; validate URIs map under open roots when applying jumps (allow absolute paths outside root if file exists — rust-analyzer stdlib / crates.io sources).

### Alternative paths

| Failure | Behavior |
|---------|----------|
| Spawn fails | Server Failed; status; no retries loop (avoid thrash) |
| Request timeout | Drop; optional log |
| File deleted at def target | open fails → status |
| Workspace switch | Keep that workspace's servers, open documents, diagnostics, and NavStack alive; activation is not workspace closure |
| Workspace removal / app exit | Send shutdown/exit, terminate remaining child if needed, clear that workspace's LSP and navigation state |

---

## Architecture sketch

```
┌─────────────────────────────────────────────────────────┐
│ xenon_ui (XenonApp)                                     │
│  - LspHost entity / owner                               │
│  - NavStack per workspace (⌘[ / ⌘])                     │
│  - definition pick list (multi-result)                  │
│  - open_editor_at, actions, status                      │
└───────────────┬─────────────────────┬───────────────────┘
                │                     │
                ▼                     ▼
        xenon_editor              xenon_lsp
        - Buffer events           - JSON-RPC
        - decorations paint       - process per family
        - ⌘-click / F12 hook      - utf-16 convert
                │                     │
                └──────── workspace root + path ──┘
```

**Crate deps:** `xenon_ui` → `xenon_lsp` + `xenon_editor`.  
`xenon_lsp` does **not** depend on gpui.  
`xenon_editor` may depend on small shared types only (or UI maps host diagnostics into view) — prefer **UI pushes decoration state into view** to keep editor free of process code.

---

## Implementation stages (for the implementing agent)

| Stage | Deliverable | Gate |
|-------|-------------|------|
| 1 | `xenon_lsp` crate: framing, bidirectional request dispatch, initialize/shutdown, runtime-neutral futures/events, mock or rust-analyzer smoke | unit + manual spawn |
| 2 | Host wiring: workspace open/close, settings overrides, PATH resolve | missing binary quiet |
| 3 | Document sync from open editors | ra sees buffer |
| 4 | `open_editor_at` + definition + multi pick list + NavStack on ⌘[ / ⌘] + F12/gd/⌘-click | G1, G4, G5 |
| 5 | documentHighlight + paint decorations | G3 |
| 6 | publishDiagnostics + underline/gutter + F8 | G7 |
| 7 | TypeScript resolve order (TLS then vtsls) + same flows | G2 |
| 8 | Theme keys, status copy, tests for UTF-16 + NavStack | G9 |
| 9 | `project install` | G10 |

Do not interleave completions or rename.

---

## Theme / paint keys (proposal)

| Key | Use |
|-----|-----|
| `editor.document_highlight.read` | occurrence (read) |
| `editor.document_highlight.write` | occurrence (write) — fallback to read color if unset |
| `editor.diagnostic.error` | underline / gutter |
| `editor.diagnostic.warning` | underline / gutter |

Map into existing theme JSON with sensible defaults derived from existing error/warning colors if present; else hardcode quiet fallbacks in code.

---

## Risks

| Risk | Mitigation |
|------|------------|
| UTF-16 bugs → wrong jump column | tests + shared convert helpers |
| Full-doc didChange lag on huge files | debounce; later incremental |
| rust-analyzer CPU | one server per workspace; stop on close |
| TS server not installed | same quiet path as rust |
| Scope creep into IDE parity | out-of-scope table is binding |
| Stale async results move cursor or paint wrong ranges | request ids + version checks; reject old versioned diagnostics; clear paints on edit |
| ⌘[ loses “back to terminal” | unified `NavEntry` includes terminal tabs; keep visit hooks |
| Multi-def pick list UX thin | reuse elevated palette chrome; path + line enough for v1 |
| Server waits forever for a client response | dispatch server requests; respond with supported result or JSON-RPC error |

---

## Consistency check

| Check | Result |
|-------|--------|
| Entities in flows defined in model | Yes: LspHost, NavStack, NavEntry, DefinitionCandidate, DiagnosticSet |
| Contracts ↔ schema | Settings only; no orphan tables |
| Side effects | Spawn process, bidirectional RPC, open tabs, paint, pick list — all in contracts/flows |
| Scope | Completions/rename/problem panel stay out |
| Open questions | All settled: B discovery, TLS→vtsls, ⌘[ NavStack, multi-def pick list, minimal diagnostics |
| Navigation invariant | Current entry is `stack[index]`; A → B appends B, Back selects A, Forward selects B |
| Runtime boundary | `xenon_lsp` exposes runtime-neutral futures/events; only `xenon_ui` creates GPUI tasks |
| Lifecycle | Workspace activation retains state; workspace removal/app exit shuts it down |
| Protocol freshness | Negotiated capabilities gate requests; stale versioned diagnostics are rejected |
| N/A | Full web/mobile augmentations N/A (native desktop GPUI app) |

---

## Non-goals reminder (product)

Xenon remains an **agent shell**. LSP makes the **editor surface** trustworthy for reading and jumping through code agents produce. It is not a commitment to VS Code language feature parity or to replacing agent-driven edits with IDE refactor UIs.
