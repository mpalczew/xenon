# Phone remote

A web page served by Xenon lets you read and drive your terminals from a phone. Code:
`crates/xenon_remote` and `crates/xenon/src/remote*.rs`.

## Decisions

- **Tailnet only.** The server listens on the Tailscale address and loopback, never the
  local network. It speaks plain HTTP, so on shared Wi-Fi anyone could read the device
  token. Don't add a LAN fallback; task password prompts rely on this too.
- **Terminals only.** No editors, diffs or file views on the phone.
- **The phone never moves the computer.** Opening a workspace or tab, starting a task,
  or closing a tab from the phone doesn't change what the computer shows.
- **Pairing code in the URL fragment,** so it never reaches a request line or log. It
  stays valid after the sheet closes, because a home-screen app has its own storage and
  the user types the code there.
- **The phone keeps its own light or dark.** Programs pick colors for the computer's
  background, so the page lifts any text color below 4.5:1 contrast.
- **Send waits before Enter,** so a program that detects pastes (Codex) doesn't swallow
  it.
- **A phone resizes a tab only while the computer is idle on it** (30 seconds). Typing
  on the computer gives the size back; with several phones, the smallest wins.

## Limits

- The phone always shows a cursor: VTE doesn't say whether the program hid it.
- Taps are clicks only while the program has mouse mode on. VTE has no getter, so the
  computer guesses from the pointer shape.
- No TLS, so no push notifications or offline mode.
