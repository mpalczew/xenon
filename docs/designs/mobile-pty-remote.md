# Tech design: Mobile PTY remote (viewport + keys)

**Status:** draft  
**Product:** Xenon open on Mac; phone drives the same terminal tabs  
**Date:** 2026-07-24  
**Shape:** lean

---

## Stage 0: Codebase context

| Piece | Path | Notes |
|-------|------|--------|
| `WorkspaceId` | `xenon_core::ids` | workspace identity |
| `TabId` / `TabState::Terminal { id, cwd }` | `xenon_core::content` | terminal tabs only for this feature |
| `TerminalView` | `xenon_terminal::view` | `send_text` / `inject_text` → PTY `input` bytes |
| Grid size | `xenon_terminal::grid` | `terminal.set_size` on **desktop** layout only |
| IDE WS + token | `xenon_ide` | pattern: localhost server + auth token; **not** the product API for phone |
| Unix IPC | `xenon` `main` | second process → app; phone needs TCP/HTTP, not unix socket |

**Gaps:** no HTTP/WS surface for remote UI; no terminal frame export API; no public “list terminals by workspace.”

**Similar:** Claude IDE lockfile + token auth (discovery + auth pattern only).

---

## Scope

**Problem:** From a phone, see the live desktop PTY viewport and type into it, without owning layout or resize.

**In scope (v1):**

1. Local HTTP(+WS) server inside Xenon (bind configurable; default Tailscale/LAN reachable, not public internet).
2. Auth: bearer/token (generated or pasted once; rotate/disable from Mac).
3. **Selector:** list workspaces → list **terminal** tabs (id, title/cwd, active hint).
4. **Viewport stream:** updating **screenshot of the visible PTY grid** (cols×rows as on Mac). Not scrollback. Not full chrome.
5. **Keyboard entry:** phone text/keys → inject into selected tab’s PTY (`inject_text` / same path as desktop).
6. **Phone zoom only:** pinch/CSS scale on the client. **Never** call PTY resize from the phone.
7. Client: mobile Safari (minimal HTML/JS served by Xenon, or static assets embedded).

**Out of scope (deferred):**

| Item | Why |
|------|-----|
| Scrollback / history document | Explicitly not v1; viewport only |
| Phone-driven PTY resize | Would fight desktop layout and agent UIs |
| Editors, sidebar, finder, full Xenon chrome | Terminals only |
| True terminal emulator on phone (xterm.js, CSI client) | Screenshot is the surface |
| Mouse/clicks on grid, paste special, file open | Keys + text first |
| Multi-phone sessions, presence, conflict UI | Single operator assumed |
| Public internet / reverse proxy / Cloudflare | Tailscale (or LAN) assumed |
| Native iOS app | Safari v1 |
| Create/close/split tabs from phone | Select existing only |

**Done means:** With Xenon open and remote enabled, phone on Tailscale opens URL → authenticates → picks workspace + terminal → sees updating viewport matching Mac → types and input appears in that PTY → Mac cols/rows unchanged when phone zooms or rotates.

**Open questions:**

| Q | Default if unset at implement |
|---|-------------------------------|
| Frame = PNG raster vs ANSI-colored cell grid JSON? | Prefer **cell grid text + attrs** if cheap (readable + light); fall back to **PNG of grid** if cell export is hard. Still “screenshot of window,” not scrollback. |
| Push (WS) vs poll? | WS push on wakeup/throttle; poll fallback OK for v1 spike. |
| Bind address / port / enable UI | Settings or menu toggle; off by default. |

---

## UX

**Entry:** Mac enables “Mobile remote” → shows URL + token (or QR). Phone opens URL on Tailscale.

**Primary flow:**

1. Open page → enter token (or `?token=` once; prefer not persist in URL long-term).
2. Workspace list → pick one.
3. Terminal list for that workspace → pick one.
4. Main view: live viewport image/grid (pinch-zoom, pan if zoomed).
5. Focus text field / hardware keyboard → keystrokes and text go to that PTY.
6. Switch terminal/workspace via selector without killing the server.

**States:**

| State | Behavior |
|-------|----------|
| Disabled / no server | Mac UI says off; phone fails connect |
| Bad/missing token | 401; no tab list |
| No terminals | empty list message |
| Selected tab closed on Mac | drop selection; re-list |
| PTY not ready | placeholder “spawning…” |
| Network blip | reconnect WS; last frame stays until new |

**Visual (phone):** top bar = workspace/terminal labels + back to lists; body = monospaced viewport (or image); bottom = input bar (text + send, optional Ctrl/Esc chips later). Zoom is browser gesture on the viewport, not server resize.

**Browser:** modern mobile Safari. No desktop-responsive redesign of Xenon.

---

## Domain model

**New (session-local, not durable product state):**

- `RemoteServer` — bound addr, token hash, enabled flag.
- `RemoteSession` — authenticated phone connection(s); optional `selected: Option<(WorkspaceId, TabId)>`.
- `ViewportFrame` — snapshot of **visible** grid only:
  - `workspace: WorkspaceId`
  - `tab: TabId`
  - `cols, rows: u16` (desktop sizes; informational)
  - `seq: u64` (monotonic)
  - payload: cells **or** image bytes
- `KeyInject` — `tab: TabId` + `text: String` (UTF-8; special keys as escapes or named tokens in contract).

**Invariants:**

1. Phone never sends cols/rows that change PTY size.
2. Frame is always **current viewport**, not scrollback buffer.
3. Only `TabState::Terminal` appears in lists.
4. Inject only if tab still exists and is Ready; else error, no create.

**Modified:** none of core session schema required for v1 (token may live in settings or ephemeral memory).

**State:** server off → on; session unauth → auth; selection none → (ws, tab); tab gone → none.

---

## Contracts

Transport: **HTTP only** (lean stack). No WebSocket.

### `GET /`

- Static mobile UI: selector + edge-to-edge terminal **PNG** + inject form.

### `POST /auth`

- Input: `{ "token": string }`
- Output: `{ "ticket": string }` (Bearer for subsequent calls)
- Errors: 401

### `GET /api/workspaces`

- Auth required
- Output: open + recent closed workspaces  
  `[{ "id", "name", "open", "root" }]`

### `GET /api/workspaces/:id/terminals`

- Auth required; **activates/reopens** workspace if needed
- Output: `[{ "tabId", "title"?, "cwd", "active" }]` — terminals only

### `GET /api/frame?workspaceId=&tabId=&since=`

- Auth required
- **200** `image/png` of visible viewport only; headers `X-Xenon-Seq`, `X-Xenon-Cols`, `X-Xenon-Rows`, `X-Xenon-TabId`
- **204** if `since` equals current seq (no change)
- Phone polls ~200ms; displays as full-width `<img>` (no horizontal scroll)

### `POST /api/inject`

- Auth required
- Body: `{ "workspaceId", "tabId", "text" }`
- Injects into PTY; **never** resizes

**Auth:** shared secret token; Tailscale is network perimeter, not a substitute for token.

---

## Schema

**N/A for product DB.** Session JSON / workspace layout unchanged.

Optional: `AppSettings` fields `remote_enabled`, `remote_port`, `remote_token` (or token only in memory + regenerate). If persisted token, store hashed or in keychain later; v1 may keep token in memory and show once.

---

## Data flow

### View

1. Phone `select` → server resolves `(WorkspaceId, TabId)` → finds live `TerminalView`.
2. On terminal wakeup / interval throttle → capture **visible** grid (or rasterize) → `ViewportFrame` with `seq++` → WS `frame`.
3. Phone replaces prior frame (no local scrollback buffer required). Pinch-zoom is CSS only.

### Type

1. Phone `input` `{ tabId, text }` → authz + tab exists → gpui update → `inject_text` → PTY bytes.
2. Desktop terminal paints as usual; next frame(s) show echo/output.
3. Failure: tab gone → WS error + client returns to list.

### Trust

- Token checked before any list/frame/input.
- Bind not `0.0.0.0` without explicit opt-in if we default to Tailscale IP or localhost+proxy; document “Tailscale only.”
- No PTY resize path from remote handlers.

**Async:** WS I/O off UI thread; inject and frame capture hop onto gpui as today for terminal access.

---

## Consistency check

| Check | Result |
|-------|--------|
| Entities used in contracts defined | `WorkspaceId`, `TabId`, `ViewportFrame`, `KeyInject`, `RemoteServer` |
| No phone resize | Protocol has no resize; UX zoom client-side |
| No scrollback | Frame = viewport only; out of scope lists history |
| Terminals only | List API filters `TabState::Terminal` |
| Schema N/A | No session layout migration |
| Side effects | Inject → existing PTY input; frames are read-only export |
| Scope creep | Editors, chrome, multi-user, public net deferred |

---

## Implement sketch (not a PR plan)

1. Export visible grid (or PNG) from terminal entity; unit-test seq + size.
2. `inject_text` path reachable from non-view caller with `TabId`.
3. Local server + token + static mobile page.
4. List workspaces/terminals; WS select + frame + input.
5. Dogfood: Tailscale URL, type from phone, confirm Mac cols/rows stable.

---

## Settled decisions (from product chat)

- Model is **updating viewport screenshot**, not a document, not last-N scrollback, not full xterm on phone.
- **Phone does not resize** the PTY; **phone may zoom** the image/grid.
- **Terminals only**; workspace + PTY selector + keyboard entry.
- Reachability: **Tailscale OK**; Xenon-native server (not third-party shell relay).
