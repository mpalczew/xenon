# Tech design: Mobile remote v2 (phone as a real Xenon client)

**Status:** implemented in `crates/xenon_remote` (server, protocol, pairing) and
`crates/xenon_gtk/src/remote*.rs` (the GTK side). This page keeps the scope, contract
and decisions. Not in the GTK app yet: tasks, mouse clicks, dim text, OSC 8 links,
cursor hiding, the phone's own light/dark choice (`.xenon/worklist.md`). **Date:**
2026-09-29

---

## Stage 1: Scope

**Problem:** Away from the desk (gym, cellular), I can't find, connect to, read, or
drive my running agents from my phone, so I never use the remote.

**In scope (v2):**

1. **Reach** — tailnet only: listen on the Tailscale/CGNAT interface IP (100.64.0.0/10,
   which also covers Headscale) plus loopback, nothing else. There is no LAN option.
   Detect it by interface, not CLI. Why: the server speaks plain HTTP, so on a shared
   Wi-Fi anyone could read the device token and rewrite the page; Xenon is open source
   and cannot depend on a hosted relay, so the encrypted transport is the user's own
   tailnet (Tailscale or Headscale).
2. **Discover / connect** — persisted enabled state; `Connect Phone…` command +
   keybinding opens a pairing sheet with QR; sidebar status indicator (off / on / phone
   connected); optional keep-awake while a phone is connected (opt-in, off by default).
3. **Read** — cell-grid frames (text + resolved RGB + attrs + cursor) rendered as real
   text on the phone; WebSocket push when the terminal changes; scrollback on demand.
4. **Fit** — phone-sized PTY while the phone drives (Decision A).
5. **Type** — key strip (Esc, ^C, Tab, ⇧Tab, ↑↓←→, 1 2 3, ⏎) via named keys; text field
   with send-with/without-Enter.
6. **App feel** — installable PWA (manifest, icon, standalone); durable per-device
   login; workspace list shows working/attention dots so I can see which agent needs me.
7. **Security** — no token in logs, no `ACAO: *`, auth rate limit, WS Origin check,
   per-device revocable tokens (hashed at rest).

**Out of scope (deferred):**

| Item                                       | Why                                                                                                             |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
| TLS / HTTPS                                | Tailscale already encrypts; `tailscale serve` HTTPS is a later add-on. Blocks Web Push + service worker (below) |
| Web Push notifications ("agent needs you") | Needs HTTPS + service worker on iOS. Next after TLS                                                             |
| Offline / service worker                   | Same HTTPS dependency; no value without it                                                                      |
| Public internet / relay                    | Tailscale assumed                                                                                               |
| Native iOS app                             | PWA first                                                                                                       |
| Editors, file view, diff review on phone   | Terminals only                                                                                                  |
| Create / close / split tabs from phone     | Select existing only (unchanged from v1)                                                                        |
| Multi-phone conflict UI                    | Single operator; multiple devices allowed, last input wins                                                      |
| Mouse / tap-to-position in grid            | Keys + text first                                                                                               |
| Image / sixel / hyperlink rendering        | Text grid only                                                                                                  |

**Done means:**

- Fresh launch with remote previously on → server is up, sidebar shows it, Mac doesn't
  idle-sleep.
- From the Mac: one keystroke → QR on screen. Phone camera → page opens logged in. "Add
  to Home Screen" → icon launches straight into workspace list, still logged in after
  Mac restart and after 7 days.
- Over cellular + Tailscale: workspace list shows which agents are working / waiting.
  Open one → crisp colored text matching the Mac, cursor visible, Unicode and
  box-drawing correct, scroll back through history.
- Claude Code permission prompt answerable with key strip; ^C interrupts; Esc cancels;
  ⇧Tab cycles mode.
- Idle terminal: zero frames sent (verified by counter). Busy terminal: ≤ 30 frames/s,
  row deltas only.
- Port is never bound on a LAN or wildcard address, only on the tailnet IP and loopback.
- `scripts/health` green; visual tests updated for pairing sheet, sidebar indicator,
  phone session page.

**Open questions:** see "Decisions" at the end (B needs a spike).

---

## Stage 2: UX

**Mac entry points**

- Command palette: **Connect Phone…** (enables remote if off, opens pairing sheet).
  Keybinding: `cmd-shift-m`-class, final pick checked against `docs/keyboard-first.md`.
- Existing **Toggle Mobile Remote** stays (no sheet).
- Sidebar footer indicator (only when enabled): phone glyph, muted = listening, accent =
  ≥1 device connected. Enter/click opens the pairing sheet. Tooltip: "Phone remote on ·
  1 device connected".
- Settings › Remote: toggle, a read-only "Network: Your tailnet only" row, keep-awake
  opt-in, QR, URL copy, paired devices list with Revoke, Rotate password.

**Pairing sheet (Mac, modal, keyboard-first)**

```
┌ Connect Phone ─────────────────────────┐
│  ▓▓▓▓▓▓▓▓▓   Scan with your phone camera│
│  ▓▓ QR ▓▓▓   http://mac.tail1234.ts.net │
│  ▓▓▓▓▓▓▓▓▓   :17890                     │
│              Then Share › Add to Home   │
│  ⚠ Tailscale not detected (if so)       │
│  [Copy link ⌘C]            [Done ⎋]     │
└─────────────────────────────────────────┘
```

QR encodes `http://<tailscale-host>:<port>/pair#<one-time pairing code>` (fragment →
never sent in request line, never logged by proxies). Pairing code is single-use, 10-min
TTL; the long-lived password never leaves the Mac in a URL.

**Phone flow**

1. Scan QR → page reads fragment → `POST /pair` → device token saved to `localStorage` →
   fragment stripped from URL.
2. Banner (Safari only, until dismissed): "Add to Home Screen for one-tap access."
3. Home-screen launch: separate iOS storage from Safari → shows "Pair this app": enter
   6-digit code shown on Mac pairing sheet (same one-time code, typeable). One time
   only.
4. **Workspaces:** open workspaces with dot (● working / ◉ needs you / none), sorted
   needs-you first. Closed workspaces under a collapsed "Recent" section.
5. **Terminals:** tabs with title + dot; if a workspace has exactly one terminal, skip
   straight into it.
6. **Session:** header (← · workspace › tab · connection dot); grid fills width; bottom:
   key strip (horizontally scrollable) + input field with ⏎ toggle.
7. Scroll up → history pages load; "↓ Live" pill returns to bottom and resumes follow.

**States**

| State                                   | Phone                                                                                  | Mac                                     |
| --------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------- |
| Remote off                              | Page unreachable (browser error)                                                       | No indicator                            |
| On, no devices                          | —                                                                                      | Indicator muted                         |
| Device connected                        | Live                                                                                   | Indicator accent                        |
| Token revoked / password rotated        | "Pair again" screen                                                                    | Device gone from list                   |
| WS drop (tunnel blip, app backgrounded) | Grey "Reconnecting…", last frame kept, exp. backoff ≤ 5s, resume on `visibilitychange` | —                                       |
| Tab closed on Mac                       | "Terminal closed" → back to terminal list                                              | —                                       |
| PTY not ready                           | "Starting…"                                                                            | —                                       |
| Too many auth failures                  | 429 "Try again in a minute"                                                            | Log line (no secret)                    |
| Tailscale absent                        | —                                                                                      | Sheet + Settings warning; loopback only |

**Visual (phone)**: colors come from the Mac theme for the page's appearance (Auto =
device `prefers-color-scheme`, or forced Dark/Light via the options-row button; the Mac
resolves the saved dark/light theme when the active one differs, `appearance` / `theme`
messages); `ui-monospace` (SF Mono). Font ~13px; phone computes cols/rows from its
viewport and sends `fit` on attach and on rotate/resize.

---

## Stage 3: Domain model

**New (in `xenon_remote`, pure where possible)**

- `CellFrame` — one terminal snapshot. `cols: u16, rows: u16, cursor: Cursor, lines:
  Vec<Line>, seq: FrameSeq`. Invariant: `lines.len() == rows`.
- `Line` — `spans: Vec<Span>`; adjacent cells with identical style merged. Trailing
  default-style blanks trimmed.
- `Span` — `text: String, style: StyleId`.
- `Style` — `fg: Rgb, bg: Rgb, flags: StyleFlags (bold|italic|underline|dim|strike)`.
  Inverse resolved on host (fg/bg swapped). Interned per connection → `StyleId`.
- `Cursor` — `row, col: u16, shape: Block|Bar|Underline, visible: bool`.
- `FrameDiff` — pure: `(prev: Option<&CellFrame>, next: &CellFrame) ->
  Option<FramePatch>`; `None` when identical. `FramePatch { seq, cols, rows, cursor,
  changed: Vec<(u16, Line)>, full: bool }`. `full` when dims change or no prev.
- `NamedKey` — `Esc | CtrlC | Tab | ShiftTab | Up | Down | Left | Right | Enter |
  Backspace`. Sent to the terminal as keys (app-cursor aware).
- `DeviceId` (uuid), `DeviceToken` (32 random bytes, base64url; only SHA-256 stored),
  `PairingCode` (6 digits + 128-bit URL secret; single use; TTL 10 min).
- `Device` — `id, label (from UA, e.g. "iPhone Safari"), token_hash, created_at,
  last_seen_at`.
- ~~`RemoteNetwork`~~ — removed (2026-10-24): tailnet-only, no choice to model. A saved
  `"remote_network"` key (including `tailscale_and_lan`) is ignored on read, with no
  error and no prompt.
- `AttachId` — per-WS-connection subscription handle on host.
- `PhoneFit` — `cols, rows: u16` requested by an attach. Invariant: clamped 20..=300 ×
  8..=200.

**Modified**

- `AppSettings` — add `remote_enabled: bool`, `remote_keep_awake_while_connected: bool
  (default false; the earlier`remote_keep_awake`key is ignored, not migrated)`.
  `remote_hostname` kept (override for URL).
- `MobileRemoteInfo` (global) — add `connected_devices: usize`, `tailscale:
  Option<TailscaleAddr>`, `pairing: Option<PairingCode>`.
- `HostRequest` — remove `CaptureFrame`; add `Attach`, `Detach`, `Key`, `History`,
  `Pair` (see contracts).

**Relationships:** one `Device` : many WS connections over time; one WS connection : ≤ 1
`Attach` (tab) at a time.

**Transitions**

- Server: `Off → Listening` (enable / launch with `remote_enabled`) → `Off` (disable).
  Keep-awake hold is held iff `Listening && remote_keep_awake_while_connected &&
  connected phones >= 1`: acquired on the first connection, released on the last
  disconnect, when the setting is turned off, or when the remote stops.
- Pairing: `None → Issued` (sheet opened) → `Consumed` (phone pairs) or `Expired` (10
  min / sheet closed... stays valid until TTL so typing on a PWA works after closing
  sheet).
- Device: `Paired → Revoked` (Settings revoke, or password rotate revokes all).
- Attach: `Detached → Attached(ws, tab)` → `Detached` on WS close, tab close, or
  re-attach.
- Tab size: `Desktop → PhoneFit` when an attach has sent `fit` and the Mac has had no
  keyboard input on that tab for 30 s → `Desktop` on last detach or Mac keyboard input
  to that tab. Multiple phones on one tab: smallest fit wins.

---

## Stage 4: Contracts

All HTTP served from the existing hand-rolled server; `/ws` upgrade routed by peeking
the request line, then handed to `tungstenite::accept` on the same `TcpStream`.

**Removed:** `GET /api/frame`, `POST /api/inject`, `POST /auth`, `?token=` bootstrap.

**New: `POST /pair`**

- In: `{ "code": "<6 digits | url secret>", "label": "iPhone Safari" }`
- Out: `200 { "deviceId", "token" }` · `401` bad/used/expired · `429` rate-limited.
- Rate limit: 5 failures / IP / minute, global 20 / minute.

**Modified: `GET /api/workspaces`**

- Auth: `Authorization: Bearer <device token>`.
- Out: adds `dot: "working" | "attention" | null`, `section: "open" | "recent"`.

**Modified: `GET /api/workspaces/{id}/terminals`**

- Adds `dot` per tab. Opening a closed workspace must not change the Mac's active
  workspace (restore previous active after reopen).

**New: `GET /api/workspaces/{id}/tasks`** — the workspace's tasks.json, for the phone's
Tasks list.

- Bearer auth like the other lists; opens a closed workspace without changing the Mac's
  active one.
- Out: `[{ "label", "detail"?, "command", "group"?: "build"|"test", "default"?: true,
  "unsupported"?: "reason" }]`. `command` is the line that will run (variables expanded
  where known, `${input:..}` as written); hidden tasks are omitted.

**New: `GET /manifest.webmanifest`, `/icon-192.png`, `/icon-512.png`,
`/apple-touch-icon.png`** — static, embedded, no auth.

**New: `POST /api/uploads`** — phone image attach (📎 or paste).

- Bearer auth; raw image body, `Content-Type` PNG/JPEG/GIF/WebP/HEIC (else 415). Body
  cap 25 MB, granted only after the token checks out (strangers keep the 8 KB cap →
  413).
- Saved as `<data dir>/remote/uploads/phone-<unix>-<id>.<ext>`; files older than 7 days
  pruned on each save. Returns `{"path"}`; the page inserts it into the message field
  unsent, so any agent that reads image paths gets it.

**New: `GET /ws` (WebSocket)**

Established sockets use TCP_NODELAY and readiness-driven I/O. The bounded host outbox
wakes the socket thread immediately; it does not wait for a read timeout. Pending writes
resume on writable readiness, with a bounded transport buffer.

- Origin must equal `http://<Host header>`; else 403.
- First client msg must be `{"t":"hello","token"}` within 5s, else close 4401.

Client → server:

```json
{"t":"hello","token":"…"}
{"t":"attach","workspaceId":"…","tabId":7}
{"t":"text","data":"fix the test"}          // raw, no implicit Enter
{"t":"key","key":"enter|shift-enter|esc|ctrl-c|tab|shift-tab|up|down|left|right|backspace"}
{"t":"history","before":-40,"count":200}    // lines above viewport, negative = scrollback
{"t":"fit","cols":52,"rows":30}             // phone viewport in cells; Decision A
{"t":"runTask","workspaceId":"…","label":"build"}   // same path as the Mac's Run Task picker
{"t":"taskInput","token":4,"value":"…"}     // answer to a taskInput; no value cancels the run
```

Server → client:

```json
{"t":"ready","theme":{"bg":"#0b0b0c","fg":"#d8d8d8"},"device":"…"}
{"t":"styles","add":{"3":{"fg":"#…","bg":"#…","f":1}}}   // interned, sent before first use
{"t":"frame","seq":12,"cols":120,"rows":40,"full":false,
 "cursor":{"r":39,"c":2,"s":"block","v":true},
 "lines":[[39,[["❯ ",3],["fix",0]]]]}
{"t":"history","before":-40,"lines":[[["…",0]]],"more":true}
{"t":"dots","workspaces":{"<id>":"attention"}}          // pushed on change
{"t":"closed","reason":"tab-closed|revoked|server-stopping"}
{"t":"taskInput","token":4,"id":"who","description":"…","default":"world","kind":"text|password|pick","options":[{"label":"…","value":"…"}]}
{"t":"taskStarted","workspaceId":"…","label":"build","tabId":9}   // attach to this tab
{"t":"error","msg":"…"}
```

Task runs: `runTask` resolves, orders (`dependsOn`) and presents exactly like the Mac
picker and runs against the main window; it never changes the Mac's tab or focus.
Questions go only to the connection that started the run and are answered with that
token from that connection (anything else is ignored). A `password` answer is masked on
the page, cleared once sent, never stored, and `Debug`-redacted on the Mac. folderOpen
approval is Mac-only; a paired device can already type into terminals, so running tasks
adds no privilege. See `docs/designs/vscode-tasks.md`. Close codes: `4401` unauthorized,
`4404` tab gone, `4409` replaced (same device attached elsewhere → allowed; not used
v2).

**Compat:** v1 phones get 404 on `/api/frame` → page reload serves v2 page → "Pair
again". No migration of tickets (they were in-memory).

---

## Stage 5: Schema

**`~/.xenon/settings.json`** — add, all `#[serde(default)]`:

```json
"remote_enabled": false,
"remote_keep_awake_while_connected": false
```

`remote_password` retained as the root secret (rotate = revoke all devices). No longer
displayed or put in URLs.

**New `~/.xenon/remote_devices.json`** (atomic write via `xenon_store`, mode 0600):

```json
{
  "devices": [
    {
      "id": "uuid",
      "label": "iPhone Safari",
      "token_sha256": "hex",
      "created_at": 1759000000,
      "last_seen_at": 1759100000
    }
  ]
}
```

- Lookup by hashing presented token and constant-time compare against ≤ ~10 rows (no
  index needed).
- `last_seen_at` written at most once per hour per device (avoid write churn).
- No expiry by default; revoke is explicit. Rotate password clears the file.

**Migration:** none needed; missing file = no devices. Pairing codes are memory-only.

---

## Stage 6: Data flow

**Trust boundaries:**

- Listener: bound only to the Tailscale/CGNAT IP (100.64.0.0/10) and loopback; never a
  LAN or wildcard address. Plain HTTP is safe only because the tailnet (Tailscale or
  Headscale, WireGuard) encrypts the path; a LAN listener would expose the device token
  and let anyone on the Wi-Fi rewrite the page. Tailscale ACLs are the outer gate.
  Password prompts for tasks (`inputs`) rely on the same boundary, so no LAN fallback or
  per-prompt refusal exists.
- `/pair`: rate-limited, single-use code.
- `/ws` + `/api/*`: device token (hash lookup) before any host request; Origin check on
  upgrade.
- Host validates `workspace_id`/`tab_id` exist on every message; PTY resize only via
  `fit`, clamped to 20..=300 cols / 8..=200 rows, and only while the Mac-idle rule
  holds.

**Alternative paths:**

- Tab closed on Mac → subscription sees view release / `is_exited` →
  `closed{tab-closed}` → phone returns to list.
- Remote disabled → server stops → `closed{server-stopping}` to all → keep-awake hold
  released.
- Tailscale IP changes (re-login) → detected on next enable/launch; pairing sheet
  reflects current. (Watching interface changes: deferred.)
- Attention dots: attention state changes → host broadcasts `dots` to all authenticated
  connections (not just attached).

---

## Decisions (2026-09-29)

**A. Phone-width text → resize to phone.** Phone sends `fit {cols, rows}` for its screen
at a readable size (~45×30 portrait). Host applies a size override to that tab's PTY
while ≥1 phone is attached *and* the Mac has had no keyboard input on that tab for 30 s.
Override released on detach or on Mac keyboard input to that tab (phone then sees the
desktop size, fit-to-width, until the Mac goes quiet again). The desktop draws the
narrow grid top-left with an empty margin. Reverses v1's "phone never resizes"
invariant.

**C. Closed workspaces → keep, collapsed "Recent" section.** Reopening from the phone
must not change the Mac's active workspace.

**D. Key row → two rows: `Esc ^C Tab ⇧Tab` / `← ↑ ↓ → 1 2 3`, tall `⏎` spanning both on
the right.** All keys visible (a single scrolling row hid 1/2/3). 1/2/3 answer Claude
Code's numbered prompts; y/n typed on the system keyboard.

**E. Mac banner while phone drives.** Terminal pane shows a one-line banner "Sized for
iPhone (45×30) · type here to take it back" above the narrow grid; tab label gets a
phone mark. Mock-reviewed 2026-09-29.

## Decision (2026-10-24): tailnet only

The LAN opt-in (`RemoteNetwork::TailscaleAndLan`, Settings › Remote › Network) is
removed. The listener binds the Tailscale/CGNAT address and loopback only. Reason: plain
HTTP on a LAN lets anyone on the Wi-Fi read the device token and rewrite the page, and
an open-source app cannot lean on a hosted relay, so the user's own tailnet (Tailscale
or Headscale) is the encrypted transport. Existing settings files that say LAN load as
tailnet-only.

Rejected:

- LAN certificates (self-signed, mkcert): each phone needs a profile install, set to
  full trust.
- In-page encryption over HTTP: `crypto.subtle` needs a secure context, and the page
  itself can be swapped.
- Vendor tunnels (VS Code, Cloudflare): the vendor can read the traffic.
- WebRTC and E2E relays (Happy, Paseo): both need hosted infrastructure.
- Native iOS app with a pinned key: a second product to build and ship.

An optional, self-hostable E2E relay could later be offered as a paid hosted add-on
alongside the tailnet path, never in place of it.

## Implementation notes (2026-09-29)

- The long-lived remote password is gone (not the "root secret" above): devices hold
  per-device tokens; "Sign out all" clears `remote_devices.json`.
- Keep-awake is a `systemd-inhibit` child. It is opt-in ("Keep this Mac awake while a
  phone is connected", off by default; the Connect Phone sheet row, ⌘K, and Settings ›
  Phone Remote). State is `Inhibitor::{Released, Held}`, synced from every published
  remote snapshot via `should_hold(remote_on, opted_in, connected)`.
- Heartbeat: the page sends `{"t":"ping"}` every 15s; the socket thread answers
  `{"t":"pong"}`. The page treats 5s of silence after a ping as a dead socket.
- Tab chip shows `title · iPhone` while a phone fit is active (no icon).
- The page sends its resolved appearance (`appearance`: dark/light, from the persisted
  Auto/Dark/Light setting) after `ready`; the Mac answers with `theme` and resolves cell
  colors against that appearance's theme. Chrome light/dark follows that theme's
  background.
- a/b slots: the second instance waits for the port and takes over within 5s of the
  first releasing it; devices are shared via `~/.xenon-shared` (`XENON_SHARED_DIR` from
  `scripts/xenon`, else inferred from the slot dir).
- Opening a closed workspace from the phone loads its terminals without activating it
  (`ensure_live_content` / `restore_closed_workspace`).
- A drag on the grid sends wheel rows through `Terminal::scroll_wheel` (the Mac trackpad
  path). The plain-text history request is no longer how the phone scrolls. "↓ Live"
  only means the terminal viewport is above the live row.
- The key tray is hidden until Keys is tapped. It floats over the grid, translucent, and
  does not resize the PTY. The iOS keyboard hides it. Send always submits the line, then
  Enter after a short pause so a paste-burst TUI (Codex) does not swallow the CR.
- New terminal (`POST /api/workspaces/:id/terminals`) appends a shell in that workspace
  without changing the Mac's workspace or selected tab.
- Close (`DELETE /api/workspaces/:id/terminals/:tab`) drops that terminal. The Mac
  changes tabs only when it was showing the one that closed.
- The title bar switches Field (edit the line, then Send) and Direct (each key goes
  straight to the program). The choice is remembered on the device. Those controls sit
  on a second row so the workspace name stays visible. Hide blurs the field and
  dismisses the software keyboard.
- A tap on the live grid is a left click at that cell, reported only while the program
  has mouse mode on. A drag still scrolls. Direct also focuses typing on that tap.
