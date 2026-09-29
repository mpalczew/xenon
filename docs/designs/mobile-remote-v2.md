# Tech design: Mobile remote v2 (phone as a real Xenon client)

**Status:** draft — decisions A–E settled; mock approved 2026-09-29; implementing
**Date:** 2026-09-29
**Supersedes:** `mobile-pty-remote.md` (v1: PNG polling, shipped as `fc3cff4`)

---

## Stage 0: Codebase context

| Piece | Path | Today |
|---|---|---|
| HTTP server | `crates/xenon_remote/src/server.rs` | Hand-rolled HTTP/1.1, thread per connection, binds `0.0.0.0:17890` |
| Auth | `xenon_remote/src/auth.rs`, `server.rs:TicketStore` | Shared password (UUID) → 12h in-memory ticket; `?token=` in URL; no rate limit |
| Host bridge | `xenon_remote/src/host.rs` `HostRequest` | `async_channel` → GPUI task → `XenonApp::handle_remote_request` (`xenon_ui/src/app/remote/mod.rs`) |
| Frame | `xenon_ui/src/app/remote/host.rs:capture_remote_frame` | `TerminalView::viewport_cells` (chars only) → `viewport_lines` → 5×7 mono PNG (`png_frame.rs`). Seq bumps every call → `since` 204 never fires |
| Input | `TerminalView::inject_text` | Page always appends `\r`; no named keys. `try_keystroke` exists (`xenon_terminal/src/view/mod.rs:609`) and respects app-cursor mode |
| Resize | `TerminalView::ensure_grid_size` | Only heals never-laid-out PTYs; desktop layout owns size |
| Attention | `xenon_ui/src/app/attention.rs` | Per-tab `Working` / `Attention(Bell|IdleSettled)` → workspace dot |
| Settings | `xenon_store/src/settings.rs:118-131` | `remote_password`, `remote_port`, `remote_hostname`. No `remote_enabled` |
| Settings UI | `xenon_ui/src/settings/remote_section.rs` | Toggle + click-to-copy URLs |
| Tailscale | `remote/mod.rs:tailscale_ipv4` | Shells `tailscale ip -4` (not on PATH for App Store Tailscale) |
| WebSocket | `tungstenite 0.24` | Already a workspace dep (`xenon_ide`) |
| Visual tests | `crates/xenon/src/visual_test_runner/remote.rs` | `remote_auth`, `remote_session` via Chrome |

---

## Stage 1: Scope

**Problem:** Away from the desk (gym, cellular), I can't find, connect to, read, or drive my running agents from my phone, so I never use the remote.

**In scope (v2):**

1. **Reach** — Tailscale-first: listen on Tailscale IP + loopback by default; LAN opt-in. Detect Tailscale by interface (100.64.0.0/10), not CLI.
2. **Discover / connect** — persisted enabled state; `Connect Phone…` command + keybinding opens a pairing sheet with QR; sidebar status indicator (off / on / phone connected); keep Mac awake while enabled.
3. **Read** — cell-grid frames (text + resolved RGB + attrs + cursor) rendered as real text on the phone; WebSocket push on PTY wakeup with row deltas; scrollback on demand.
4. **Fit** — phone-sized PTY while the phone drives (Decision A).
5. **Type** — key strip (Esc, ^C, Tab, ⇧Tab, ↑↓←→, 1 2 3, ⏎) via named keys → `try_keystroke`; text field with send-with/without-Enter.
6. **App feel** — installable PWA (manifest, icon, standalone); durable per-device login; workspace list shows working/attention dots so I can see which agent needs me.
7. **Security** — no token in logs, no `ACAO: *`, auth rate limit, WS Origin check, per-device revocable tokens (hashed at rest).

**Out of scope (deferred):**

| Item | Why |
|---|---|
| TLS / HTTPS | Tailscale already encrypts; `tailscale serve` HTTPS is a later add-on. Blocks Web Push + service worker (below) |
| Web Push notifications ("agent needs you") | Needs HTTPS + service worker on iOS. Next after TLS |
| Offline / service worker | Same HTTPS dependency; no value without it |
| Public internet / relay | Tailscale assumed |
| Native iOS app | PWA first |
| Editors, file view, diff review on phone | Terminals only |
| Create / close / split tabs from phone | Select existing only (unchanged from v1) |
| Multi-phone conflict UI | Single operator; multiple devices allowed, last input wins |
| Mouse / tap-to-position in grid | Keys + text first |
| Image / sixel / hyperlink rendering | Text grid only |

**Done means:**

- Fresh launch with remote previously on → server is up, sidebar shows it, Mac doesn't idle-sleep.
- From the Mac: one keystroke → QR on screen. Phone camera → page opens logged in. "Add to Home Screen" → icon launches straight into workspace list, still logged in after Mac restart and after 7 days.
- Over cellular + Tailscale: workspace list shows which agents are working / waiting. Open one → crisp colored text matching the Mac, cursor visible, Unicode and box-drawing correct, scroll back through history.
- Claude Code permission prompt answerable with key strip; ^C interrupts; Esc cancels; ⇧Tab cycles mode.
- Idle terminal: zero frames sent (verified by counter). Busy terminal: ≤ 30 frames/s, row deltas only.
- Port not reachable on a non-Tailscale LAN IP unless LAN opt-in is on.
- `scripts/health` green; visual tests updated for pairing sheet, sidebar indicator, phone session page.

**Open questions:** see "Decisions" at the end (B needs a spike).

---

## Stage 2: UX

**Mac entry points**

- Command palette: **Connect Phone…** (enables remote if off, opens pairing sheet). Keybinding: `cmd-shift-m`-class, final pick checked against `docs/keyboard-first.md`.
- Existing **Toggle Mobile Remote** stays (no sheet).
- Sidebar footer indicator (only when enabled): phone glyph, muted = listening, accent = ≥1 device connected. Enter/click opens the pairing sheet. Tooltip: "Phone remote on · 1 device connected".
- Settings › Remote: toggle, network (Tailscale / Tailscale + LAN), keep-awake toggle, QR, URL copy, paired devices list with Revoke, Rotate password.

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

QR encodes `http://<tailscale-host>:<port>/pair#<one-time pairing code>` (fragment → never sent in request line, never logged by proxies). Pairing code is single-use, 10-min TTL; the long-lived password never leaves the Mac in a URL.

**Phone flow**

1. Scan QR → page reads fragment → `POST /pair` → device token saved to `localStorage` → fragment stripped from URL.
2. Banner (Safari only, until dismissed): "Add to Home Screen for one-tap access."
3. Home-screen launch: separate iOS storage from Safari → shows "Pair this app": enter 6-digit code shown on Mac pairing sheet (same one-time code, typeable). One time only.
4. **Workspaces:** open workspaces with dot (● working / ◉ needs you / none), sorted needs-you first. Closed workspaces under a collapsed "Recent" section.
5. **Terminals:** tabs with title + dot; if a workspace has exactly one terminal, skip straight into it.
6. **Session:** header (← · workspace › tab · connection dot); grid fills width; bottom: key strip (horizontally scrollable) + input field with ⏎ toggle.
7. Scroll up → history pages load; "↓ Live" pill returns to bottom and resumes follow.

**States**

| State | Phone | Mac |
|---|---|---|
| Remote off | Page unreachable (browser error) | No indicator |
| On, no devices | — | Indicator muted |
| Device connected | Live | Indicator accent |
| Token revoked / password rotated | "Pair again" screen | Device gone from list |
| WS drop (tunnel blip, app backgrounded) | Grey "Reconnecting…", last frame kept, exp. backoff ≤ 5s, resume on `visibilitychange` | — |
| Tab closed on Mac | "Terminal closed" → back to terminal list | — |
| PTY not ready | "Starting…" | — |
| Too many auth failures | 429 "Try again in a minute" | Log line (no secret) |
| Tailscale absent | — | Sheet + Settings warning; loopback only |

**Visual (phone)**: theme background/foreground from Mac theme; `ui-monospace` (SF Mono); dark by default. Font ~13px; phone computes cols/rows from its viewport and sends `fit` on attach and on rotate/resize.

---

## Stage 3: Domain model

**New (in `xenon_remote`, pure where possible)**

- `CellFrame` — one terminal snapshot. `cols: u16, rows: u16, cursor: Cursor, lines: Vec<Line>, seq: FrameSeq`. Invariant: `lines.len() == rows`.
- `Line` — `spans: Vec<Span>`; adjacent cells with identical style merged. Trailing default-style blanks trimmed.
- `Span` — `text: String, style: StyleId`.
- `Style` — `fg: Rgb, bg: Rgb, flags: StyleFlags (bold|italic|underline|dim|strike)`. Inverse resolved on host (fg/bg swapped). Interned per connection → `StyleId`.
- `Cursor` — `row, col: u16, shape: Block|Bar|Underline, visible: bool`.
- `FrameDiff` — pure: `(prev: Option<&CellFrame>, next: &CellFrame) -> Option<FramePatch>`; `None` when identical. `FramePatch { seq, cols, rows, cursor, changed: Vec<(u16, Line)>, full: bool }`. `full` when dims change or no prev.
- `NamedKey` — `Esc | CtrlC | Tab | ShiftTab | Up | Down | Left | Right | Enter | Backspace`. Maps to a `gpui::Keystroke` on host → `try_keystroke` (app-cursor aware).
- `DeviceId` (uuid), `DeviceToken` (32 random bytes, base64url; only SHA-256 stored), `PairingCode` (6 digits + 128-bit URL secret; single use; TTL 10 min).
- `Device` — `id, label (from UA, e.g. "iPhone Safari"), token_hash, created_at, last_seen_at`.
- `RemoteNetwork` — `Tailscale | TailscaleAndLan`.
- `AttachId` — per-WS-connection subscription handle on host.
- `PhoneFit` — `cols, rows: u16` requested by an attach. Invariant: clamped 20..=300 × 8..=200.
- `SizeOverride` (on `TerminalView`) — `Option<(u16, u16)>`; when `Some`, the element lays out that grid instead of its bounds.

**Modified**

- `AppSettings` — add `remote_enabled: bool`, `remote_network: RemoteNetwork`, `remote_keep_awake: bool (default true)`. `remote_hostname` kept (override for URL).
- `MobileRemoteInfo` (global) — add `connected_devices: usize`, `tailscale: Option<TailscaleAddr>`, `pairing: Option<PairingCode>`.
- `HostRequest` — remove `CaptureFrame`; add `Attach`, `Detach`, `Key`, `History`, `Pair` (see contracts).
- `TerminalView` — add `cell_snapshot(&self, cx, palette) -> Option<CellFrame>` and `history_lines(before, count)` (scrollback read).

**Removed:** `png_frame.rs`, `viewport_lines` (frame.rs), `image` dep, `FrameMeta`, `/api/frame`.

**Relationships:** one `Device` : many WS connections over time; one WS connection : ≤ 1 `Attach` (tab) at a time; `Attach` → one `Entity<TerminalView>` subscription.

**Transitions**

- Server: `Off → Listening` (enable / launch with `remote_enabled`) → `Off` (disable). Keep-awake assertion held iff `Listening && remote_keep_awake`.
- Pairing: `None → Issued` (sheet opened) → `Consumed` (phone pairs) or `Expired` (10 min / sheet closed... stays valid until TTL so typing on a PWA works after closing sheet).
- Device: `Paired → Revoked` (Settings revoke, or password rotate revokes all).
- Attach: `Detached → Attached(ws, tab)` → `Detached` on WS close, tab close, or re-attach.
- Tab size: `Desktop → PhoneFit` when an attach has sent `fit` and the Mac has had no keyboard input on that tab for 30 s → `Desktop` on last detach or Mac keyboard input to that tab. Multiple phones on one tab: smallest fit wins.

---

## Stage 4: Contracts

All HTTP served from the existing hand-rolled server; `/ws` upgrade routed by peeking the request line, then handed to `tungstenite::accept` on the same `TcpStream`.

**Removed:** `GET /api/frame`, `POST /api/inject`, `POST /auth`, `?token=` bootstrap.

**New: `POST /pair`**
- In: `{ "code": "<6 digits | url secret>", "label": "iPhone Safari" }`
- Out: `200 { "deviceId", "token" }` · `401` bad/used/expired · `429` rate-limited.
- Rate limit: 5 failures / IP / minute, global 20 / minute.

**Modified: `GET /api/workspaces`**
- Auth: `Authorization: Bearer <device token>`.
- Out: adds `dot: "working" | "attention" | null`, `section: "open" | "recent"`.

**Modified: `GET /api/workspaces/{id}/terminals`**
- Adds `dot` per tab. Opening a closed workspace must not change the Mac's active workspace (restore previous active after reopen).

**New: `GET /manifest.webmanifest`, `/icon-192.png`, `/icon-512.png`, `/apple-touch-icon.png`** — static, embedded, no auth.

**New: `GET /ws` (WebSocket)**
- Origin must equal `http://<Host header>`; else 403.
- First client msg must be `{"t":"hello","token"}` within 5s, else close 4401.

Client → server:
```json
{"t":"hello","token":"…"}
{"t":"attach","workspaceId":"…","tabId":7}
{"t":"text","data":"fix the test"}          // raw, no implicit Enter
{"t":"key","key":"enter|esc|ctrl-c|tab|shift-tab|up|down|left|right|backspace"}
{"t":"history","before":-40,"count":200}    // lines above viewport, negative = scrollback
{"t":"fit","cols":52,"rows":30}             // phone viewport in cells; Decision A
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
{"t":"error","msg":"…"}
```
Close codes: `4401` unauthorized, `4404` tab gone, `4409` replaced (same device attached elsewhere → allowed; not used v2).

**Modified: `HostRequest` (xenon_remote ↔ xenon_ui)**
```rust
Attach { conn: AttachId, workspace_id, tab_id, out: Sender<ServerMsg>, reply }
Detach { conn: AttachId }
Text   { conn: AttachId, text: String }
Key    { conn: AttachId, key: NamedKey }
History{ conn: AttachId, before: i32, count: u16, reply }
Pair   { code: String, label: String, reply }   // host owns device store + pairing state
AuthDevice { token: String, reply: SyncSender<Option<DeviceId>> }
```
Host pushes frames/dots through `out`; the WS thread never polls the host for frames.

**New: `xenon_terminal::TerminalView`**
- `cell_snapshot(&self, cx: &App) -> Option<CellFrame>` — viewport cells with colors resolved through the same palette path `TerminalElement` uses; wide-char spacers skipped.
- `history_lines(&self, before: i32, count: u16, cx: &App) -> Vec<Line>`.
- `send_named_key(&mut self, key: NamedKey, cx)` — builds `Keystroke`, calls `try_keystroke`.
- Subscription: existing `terminal::Event::Wakeup` (already used for repaint) drives remote push.

**New: keep-awake (`xenon_ui` or small `xenon_platform` helper)**
- `KeepAwake::acquire(reason) -> Guard` — `IOPMAssertionCreateWithName(PreventUserIdleSystemSleep)`; drop releases.

**Compat:** v1 phones get 404 on `/api/frame` → page reload serves v2 page → "Pair again". No migration of tickets (they were in-memory).

---

## Stage 5: Schema

**`~/.xenon/settings.json`** — add, all `#[serde(default)]`:
```json
"remote_enabled": false,
"remote_network": "tailscale",
"remote_keep_awake": true
```
`remote_password` retained as the root secret (rotate = revoke all devices). No longer displayed or put in URLs.

**New `~/.xenon/remote_devices.json`** (atomic write via `xenon_store`, mode 0600):
```json
{ "devices": [
  { "id": "uuid", "label": "iPhone Safari", "token_sha256": "hex",
    "created_at": 1759000000, "last_seen_at": 1759100000 } ] }
```
- Lookup by hashing presented token and constant-time compare against ≤ ~10 rows (no index needed).
- `last_seen_at` written at most once per hour per device (avoid write churn).
- No expiry by default; revoke is explicit. Rotate password clears the file.

**Migration:** none needed; missing file = no devices. Pairing codes are memory-only.

---

## Stage 6: Data flow

**Representative path: agent prints output → phone sees it**

1. PTY bytes → zed `Terminal` → `Event::Wakeup` on the `TerminalView`'s terminal (GPUI main thread).
2. `XenonApp` remote host has a `cx.subscribe` for each attached tab (created on `Attach`). On Wakeup: mark `dirty`; if no flush scheduled, schedule one in 33 ms (coalesces bursts → ≤ 30 fps).
3. Flush: `view.cell_snapshot(cx)` → `FrameDiff(prev, next)` against that attach's last-sent frame. `None` → nothing sent (idle = zero traffic). Else intern new styles, build `ServerMsg::Frame` + optional `Styles`, `out.try_send`.
   - Backpressure: `out` is bounded (4). If full, drop the patch and set `needs_full` → next flush sends a full frame. Slow cellular can't grow memory.
4. WS thread loop (blocking socket, 50 ms read timeout): drain `out` → `tungstenite::send` (JSON text msg); then try read one client message.
5. Phone: `onmessage` → apply patch to in-memory row array → re-render only changed row `<div>`s (each row = spans with class per `StyleId`) → cursor overlay positioned by `ch`/line-height units.
6. Failure: send error → WS thread ends → drops `AttachId` → sends `Detach` → host drops subscription. Phone reconnects with backoff, re-`attach`, gets `full` frame.

**Fit path:** phone `fit` → host records `PhoneFit` on the attach → if Mac-idle rule holds, `view.set_size_override(Some(..))` → PTY `set_size` → agent reflows → Wakeup → full frame (dims changed). Mac keyboard input on that tab → `set_size_override(None)` → next layout restores desktop size.

**Input path:** key strip tap → `{"t":"key","key":"ctrl-c"}` → WS thread → `HostRequest::Key` → host resolves attach → `view.send_named_key` → `try_keystroke` → PTY. Text field send → `text` (+ `key:enter` if ⏎ toggle on).

**Async boundaries:** GPUI main thread (snapshot, diff, keystroke) ↔ `async_channel` ↔ per-connection OS thread (socket I/O). Diff runs on main thread; cost is O(rows × cols) only when dirty — acceptable at 120×40. If profiling shows jank, move diff to the WS thread by sending snapshots instead.

**Trust boundaries:**
- Listener: bound only to Tailscale IP (+ loopback; + LAN IPs iff opt-in). Tailscale ACLs are the outer gate.
- `/pair`: rate-limited, single-use code.
- `/ws` + `/api/*`: device token (hash lookup) before any host request; Origin check on upgrade.
- Host validates `workspace_id`/`tab_id` exist on every message; PTY resize only via `fit`, clamped to 20..=300 cols / 8..=200 rows, and only while the Mac-idle rule holds.

**Alternative paths:**
- Tab closed on Mac → subscription sees view release / `is_exited` → `closed{tab-closed}` → phone returns to list.
- Remote disabled → server stops → `closed{server-stopping}` to all → keep-awake released.
- Tailscale IP changes (re-login) → detected on next enable/launch; pairing sheet reflects current. (Watching interface changes: deferred.)
- Attention dots: attention state changes → host broadcasts `dots` to all authenticated connections (not just attached).

---

## Decisions (2026-09-29)

**A. Phone-width text → resize to phone.** Phone sends `fit {cols, rows}` for its screen at a readable size (~45×30 portrait). Host applies a size override to that tab's PTY while ≥1 phone is attached *and* the Mac has had no keyboard input on that tab for 30 s. Override released on detach or on Mac keyboard input to that tab (phone then sees the desktop size, fit-to-width, until the Mac goes quiet again). `TerminalView` gains a size override that `TerminalElement` respects: Mac draws the narrow grid top-left with an empty margin. Reverses v1's "phone never resizes" invariant.

**B. Scrollback → spike on-demand history against the pinned zed rev; fallback = last 1000 lines sent at attach.**

**C. Closed workspaces → keep, collapsed "Recent" section.** Reopening from the phone must not change the Mac's active workspace.

**D. Key row → two rows: `Esc ^C Tab ⇧Tab` / `← ↑ ↓ → 1 2 3`, tall `⏎` spanning both on the right.** All keys visible (a single scrolling row hid 1/2/3). 1/2/3 answer Claude Code's numbered prompts; y/n typed on the system keyboard.

**E. Mac banner while phone drives.** Terminal pane shows a one-line banner "Sized for iPhone (45×30) · type here to take it back" above the narrow grid; tab label gets a phone mark. Mock-reviewed 2026-09-29.

## Implementation notes (2026-09-29)

- Scrollback (Decision B): zed's terminal exposes only plain text beyond the
  viewport (`get_content`), so history is the last 1000 lines as plain text,
  sent once per attach on `{"t":"history"}`.
- The long-lived remote password is gone (not the "root secret" above): devices
  hold per-device tokens; "Sign out all" clears `remote_devices.json`.
- Keep-awake is a `caffeinate -i -w <pid>` child, not an IOKit assertion.
- Heartbeat: the page sends `{"t":"ping"}` every 15s; the socket thread answers
  `{"t":"pong"}`. The page treats 5s of silence after a ping as a dead socket.
- Tab chip shows `title · iPhone` while a phone fit is active (no icon).
- The page picks light/dark chrome from the Mac terminal theme's background.
