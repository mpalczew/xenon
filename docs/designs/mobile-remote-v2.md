# Phone remote

The phone remote lets you find, read and drive your running terminals from a phone, away
from the desk. The phone gets a web page (installable as a PWA) served by Xenon itself.
It shows terminals only: no editors, diffs, or file views, and it cannot split tabs.

Code: `crates/xenon_remote` (server, protocol, pairing) and
`crates/xenon/src/remote*.rs` (the GTK side). The wire format is in
`xenon_remote/src/protocol.rs`.

## Network and security

- The server listens on the Tailscale address (100.64.0.0/10, which also covers
  Headscale) and loopback, and nothing else. There is no LAN option and no TLS.
- That is deliberate. The server speaks plain HTTP, so on shared Wi-Fi anyone could read
  the device token and rewrite the page. Xenon is open source and cannot rely on a
  hosted relay, so the encrypted path is the user's own tailnet. Do not add a LAN
  fallback. Task password prompts rely on the same boundary.
- Tailscale is found by looking at network interfaces, not by calling the CLI. The
  address is read on enable and on launch; a changed address is not noticed until then.
- An old `remote_network` setting is ignored.

## Pairing and devices

- Ctrl+Shift+M or the command "Connect Phone" turns the remote on and opens a sheet with
  a QR code. The QR holds a one-time pairing code in the URL fragment, so it never
  reaches a request line or log. The code works once and expires after 10 minutes. It
  stays valid after the sheet closes, because a home-screen PWA has separate storage
  from Safari and the user types the 6-digit form there.
- Each phone gets its own token, kept hashed in `remote_devices.json`. There is no
  expiry. Revoking a device, or "Sign out all", ends access. The phone then shows "Pair
  again".
- Failed pairing attempts are rate limited per IP and globally.
- Device tokens are shared between the a and b instance slots through the
  `.xenon-shared` directory, and the second instance takes over the port within a few
  seconds of the first letting go.
- The WebSocket checks the Origin header and needs a `hello` with the token within five
  seconds.

## What the phone does

- Workspace list shows a working or needs-you dot, with needs-you first. Closed
  workspaces sit under a collapsed "Recent". Opening one from the phone must not change
  the computer's active workspace or tab. The same holds for New terminal and Close.
- A workspace with one terminal opens straight into it.
- The terminal is drawn as real text from cell frames. Only changed rows are sent, and
  an idle terminal sends nothing. Inverse is resolved on the computer. The page forces
  at least 4.5:1 contrast on every text color, because programs pick colors for the
  computer's background.
- Dragging scrolls like a trackpad. Tapping is a click only while the program has mouse
  mode on. VTE has no getter for that, so the computer infers it from the pointer shape
  (arrow instead of text cursor). Only the SGR mouse form is sent.
- Typing has two modes, remembered per device. Field: edit a line, then Send, which
  sends the line and then Enter after a short pause so a paste-detecting TUI (Codex)
  does not swallow the Enter. Direct: each key goes straight to the program. The key
  tray (Esc, ^C, Tab, Shift+Tab, arrows, 1 2 3, Enter) floats over the grid and does not
  resize the terminal. The 1 2 3 keys answer numbered prompts in Claude Code.
- The image button uploads to `<data dir>/remote/uploads` (25 MB cap, files over 7 days
  old are deleted on each save) and puts the path into the message field, unsent.
- A Tasks list shows the workspace's tasks.json. Running one behaves as on the computer
  and never changes the computer's tab or focus. Questions go only to the phone that
  started the run, and a dropped phone cancels the run. See `vscode-tasks.md`.
- OSC 8 links are rebuilt from VTE per cell and opened only if they are `http(s)` or
  `mailto`.
- The page follows a dark, light or auto setting. The computer keeps its own look.
- The page sends a ping every 15 seconds and treats 5 seconds of silence as a dead
  socket. It reconnects with backoff and keeps the last frame on screen.

## Sizing

While a phone is attached and the computer has had no keyboard input on that tab for 30
seconds, the terminal is resized to the phone's size (clamped to 20-300 columns by 8-200
rows). Typing on the computer on that tab, or the last phone detaching, restores the
desktop size. With several phones, the smallest wins. The tab's title shows the phone's
name.

## Computer side

- A sidebar indicator shows off, listening, or phone connected.
- Settings has the toggle, paired devices, sign out all, and an opt-in "keep this
  computer awake while a phone is connected" (off by default), which runs
  `systemd-inhibit` while at least one phone is connected.

## Known limits

- The phone always shows a cursor, because VTE does not say whether the program hid it.
- No TLS, so no Web Push and no service worker or offline mode.
- Several phones may connect; the last input wins.
